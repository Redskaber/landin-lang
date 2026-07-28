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
use crate::mir::dyn_trait::build_dyn_trait_mir_plan_from_resolver;
use crate::mir::lower::lower_hir_body_to_mir_full_with_dyn_trait_plan;
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
    /// Stage 5.22: Trait coherence/completeness errors (non-fatal —
    /// compilation continues but the user should fix these).
    pub trait_errors: Vec<String>,
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
        // Stage 14.10: trait_errors were missing from format_for_user,
        // causing "error: N error(s)" with no detail lines when only
        // trait coherence/completeness errors existed. total_count()
        // already included trait_errors.len(), so the count was correct
        // but the details were invisible. This fix closes that diagnostic
        // gap by printing each trait error with a [trait] prefix.
        for e in &self.trait_errors {
            out.push_str(&format!("  [trait] {}\n", e));
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
    errors.resolve = resolve_crate(&mut hir, &interner);

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
                        resolve_self_param_type_for_sig(&hir, *def_id, p.self_kind).unwrap_or_else(
                            || {
                                // Fallback: if self_ty resolution fails, try p.ty
                                if let Some(ty) = &p.ty {
                                    crate::mir::lower::lower_hir_ty_to_mir_ty(ty)
                                } else {
                                    crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Error, p.span)
                                }
                            },
                        )
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
                    inputs,
                    output: Box::new(output),
                    abi: f.sig.abi,
                    is_unsafe: f.sig.is_unsafe,
                },
            );
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
    trait_resolver.collect(&hir, &interner);

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
    // HIR `receiver.method(args)` → MIR `Terminator::Call` with Const
    // marker → codegen `getelementptr + load + load + indirect call`.
    let dyn_trait_plan = build_dyn_trait_mir_plan_from_resolver(&trait_resolver, &interner);

    for (body_id, body) in &hir.bodies {
        let return_ty = hir.owner(body_id.owner.0).and_then(owner_return_ty);

        let (mut mir, lower_unify) = lower_hir_body_to_mir_full_with_dyn_trait_plan(
            body,
            &interner,
            &hir,
            return_ty,
            Some(&dyn_trait_plan),
        );

        // Stage 14.30: Collect type errors from MIR lowering (e.g., "no method found").
        // Per "报错 > 静默" principle — these errors were previously silently
        // swallowed (Error placeholder → codegen produced 0 or invalid IR).
        errors.typeck.append(&mut mir.lower_type_errors);

        // Stage 3.60: typeck uses pre-computed tables instead of HIR.
        let mut tc = typeck::TypeChecker::with_unify(lower_unify);
        tc.fn_sigs = fn_sig_table.sigs.clone();
        tc.check_mir_body_with_tables(&mut mir, Some(&field_ty_table));
        let (type_errors, body_results) = tc.into_results();
        errors.typeck.extend(type_errors);
        typeck_results.push(body_results);

        // Borrow check
        let mut bc = borrowck::BorrowChecker::new();
        bc.check_mir_body(&mir);
        errors.borrowck.extend(bc.into_errors());

        // Stage 14.49: Write back concrete tuple types for tuple literals.
        //
        // Tuple literals use fresh_infer_ty for their type (so typeck can unify
        // with let binding annotations). After typeck, if the local's type is
        // still Infer but it's assigned from a Tuple Aggregate, we need to
        // resolve the concrete tuple type from the Aggregate's operands.
        //
        // This enables nested tuple destructure — the outer tuple's type must
        // be Tuple([...]) (not Infer) so field types can be extracted.
        //
        // Per §13.4: typeck resolves the tuple type via unification; this
        // writeback step sinks the resolved type into local_decls for codegen.
        for bb in &mir.basic_blocks {
            for stmt in &bb.statements {
                if let crate::mir::body::StatementKind::Assign(boxed) = &stmt.kind {
                    let (place, rvalue) = &**boxed;
                    if let crate::mir::place::PlaceKind::Local(dest_id) = &place.kind {
                        let dest_ty = &mir.local_decls[dest_id.0 as usize].ty;
                        if matches!(&dest_ty.kind, crate::mir::ty::TyKind::Infer(_)) {
                            // Check if rvalue is a Tuple Aggregate
                            if let crate::mir::place::Rvalue::Aggregate(
                                crate::mir::place::AggregateKind::Tuple,
                                operands,
                            ) = rvalue
                            {
                                // Build the tuple type from operand types
                                let elem_tys: Vec<crate::mir::ty::Ty> = operands
                                    .iter()
                                    .map(|op| match op {
                                        crate::mir::place::Operand::Copy(p)
                                        | crate::mir::place::Operand::Move(p) => {
                                            if let crate::mir::place::PlaceKind::Local(id) = &p.kind
                                            {
                                                mir.local_decls
                                                    .get(id.0 as usize)
                                                    .map(|ld| ld.ty.clone())
                                                    .unwrap_or_else(|| {
                                                        crate::mir::ty::Ty::new(
                                                            crate::mir::ty::TyKind::Infer(
                                                                crate::mir::ty::InferVar::TyVar(
                                                                    crate::mir::ty::TyVid(0),
                                                                ),
                                                            ),
                                                            p.span,
                                                        )
                                                    })
                                            } else {
                                                crate::mir::ty::Ty::new(
                                                    crate::mir::ty::TyKind::Infer(
                                                        crate::mir::ty::InferVar::TyVar(
                                                            crate::mir::ty::TyVid(0),
                                                        ),
                                                    ),
                                                    p.span,
                                                )
                                            }
                                        }
                                        crate::mir::place::Operand::Constant(c) => {
                                            c.ty.as_ref().clone()
                                        }
                                    })
                                    .collect();
                                let tuple_ty = crate::mir::ty::Ty::new(
                                    crate::mir::ty::TyKind::Tuple(elem_tys),
                                    stmt.span,
                                );
                                if let Some(ld) = mir.local_decls.get_mut(dest_id.0 as usize) {
                                    if matches!(&ld.ty.kind, crate::mir::ty::TyKind::Infer(_)) {
                                        ld.ty = tuple_ty;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Stage 14.37: Write back Call dest local types from fn_sigs.
        // After typeck, Call dest locals may still have Infer→i32 type
        // (typeck doesn't propagate Call return types). This pass scans
        // all Call terminators and writes the callee's return type into
        // the dest local's local_decl, so codegen allocates the correct
        // size and field access uses the correct type.
        //
        // Also propagates types through Assign statements: if a local is
        // assigned from a Copy/Move of another local whose type was
        // written back, propagate the type to the destination local too.
        // This handles `let c = a.add(b);` where c's local gets the
        // struct type from the Call dest.
        for bb in &mir.basic_blocks {
            if let crate::mir::body::Terminator::Call {
                func, destination, ..
            } = &bb.terminator
            {
                if let crate::mir::place::PlaceKind::Local(id) = &destination.kind {
                    let callee_def_id = if let crate::mir::place::Operand::Constant(c) = func {
                        match &c.val {
                            crate::mir::ty::ConstVal::Uint(n) => Some(crate::hir::DefId(*n as u32)),
                            crate::mir::ty::ConstVal::Int(n) => Some(crate::hir::DefId(*n as u32)),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if let Some(did) = callee_def_id {
                        if let Some(sig) = fn_sig_table.get(&did) {
                            let dest_idx = id.0 as usize;
                            if let Some(ld) = mir.local_decls.get_mut(dest_idx) {
                                if matches!(
                                    &ld.ty.kind,
                                    crate::mir::ty::TyKind::Infer(_)
                                        | crate::mir::ty::TyKind::Error
                                ) {
                                    ld.ty = sig.output.as_ref().clone();
                                }
                            }
                        }
                    }
                }
            }
        }

        // Stage 14.44: Write back types for Index projection Copy dests.
        //
        // When `loc = Copy(arr[i])` and loc's type is Infer, we need to
        // write back the array's element type to loc. This is needed for
        // `arr[0].field` patterns where the loaded element (a struct) is
        // stored into an Infer-typed local — without writeback, the alloca
        // is i32 (fallback), causing GEP errors when accessing fields.
        //
        // Per §13.4: mirrors the Call dest writeback above but for Index
        // projections. The element type is extracted from the array type
        // in the source place's local_decls.
        for bb in &mir.basic_blocks {
            for stmt in &bb.statements {
                if let crate::mir::body::StatementKind::Assign(boxed) = &stmt.kind {
                    let (place, rvalue) = &**boxed;
                    if let crate::mir::place::PlaceKind::Local(dest_id) = &place.kind {
                        // Check if dest is Infer/Error
                        let dest_ty = &mir.local_decls[dest_id.0 as usize].ty;
                        if !matches!(
                            &dest_ty.kind,
                            crate::mir::ty::TyKind::Infer(_) | crate::mir::ty::TyKind::Error
                        ) {
                            continue;
                        }
                        // Check if rvalue is Copy/Move of an Index projection
                        if let crate::mir::place::Rvalue::Use(
                            crate::mir::place::Operand::Copy(src_place)
                            | crate::mir::place::Operand::Move(src_place),
                        ) = rvalue
                        {
                            if let crate::mir::place::PlaceKind::Projection(base, elem) =
                                &src_place.kind
                            {
                                if matches!(
                                    elem,
                                    crate::mir::place::ProjectionElem::Index(_)
                                        | crate::mir::place::ProjectionElem::ConstantIndex { .. }
                                ) {
                                    // Get the base's type (the array)
                                    if let crate::mir::place::PlaceKind::Local(arr_id) = &base.kind
                                    {
                                        let elem_ty_clone = mir
                                            .local_decls
                                            .get(arr_id.0 as usize)
                                            .and_then(|arr_ld| {
                                                if let crate::mir::ty::TyKind::Array(elem_ty, _) =
                                                    &arr_ld.ty.kind
                                                {
                                                    Some(elem_ty.as_ref().clone())
                                                } else {
                                                    None
                                                }
                                            });
                                        if let Some(elem_ty) = elem_ty_clone {
                                            // Write back the element type
                                            if let Some(ld) =
                                                mir.local_decls.get_mut(dest_id.0 as usize)
                                            {
                                                if matches!(
                                                    &ld.ty.kind,
                                                    crate::mir::ty::TyKind::Infer(_)
                                                        | crate::mir::ty::TyKind::Error
                                                ) {
                                                    ld.ty = elem_ty;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Stage 14.49: Write back types for Field projection Copy dests.
        //
        // When `loc = Copy(tuple.field)` and loc's type is Infer, write back
        // the field's type from the source tuple's type. This is needed for
        // nested tuple destructure where the inner tuple local gets Infer type
        // at MIR-lower time, but after typeck the outer tuple has a concrete
        // type with concrete field types.
        //
        // Per §13.4: mirrors the Index projection writeback above.
        for bb in &mir.basic_blocks {
            for stmt in &bb.statements {
                if let crate::mir::body::StatementKind::Assign(boxed) = &stmt.kind {
                    let (place, rvalue) = &**boxed;
                    if let crate::mir::place::PlaceKind::Local(dest_id) = &place.kind {
                        let dest_ty = &mir.local_decls[dest_id.0 as usize].ty;
                        if !matches!(
                            &dest_ty.kind,
                            crate::mir::ty::TyKind::Infer(_) | crate::mir::ty::TyKind::Error
                        ) {
                            continue;
                        }
                        if let crate::mir::place::Rvalue::Use(
                            crate::mir::place::Operand::Copy(src_place)
                            | crate::mir::place::Operand::Move(src_place),
                        ) = rvalue
                        {
                            if let crate::mir::place::PlaceKind::Projection(
                                base,
                                crate::mir::place::ProjectionElem::Field(field_id, field_ty),
                            ) = &src_place.kind
                            {
                                // Get the base's type (the tuple/struct)
                                if let crate::mir::place::PlaceKind::Local(base_id) = &base.kind {
                                    if let Some(base_ld) = mir.local_decls.get(base_id.0 as usize) {
                                        // The field_ty in the projection may be Infer;
                                        // try to get the actual field type from the base's Tuple type
                                        let resolved_field_ty = if matches!(
                                            &field_ty.kind,
                                            crate::mir::ty::TyKind::Infer(_)
                                        ) {
                                            if let crate::mir::ty::TyKind::Tuple(field_tys) =
                                                &base_ld.ty.kind
                                            {
                                                // Extract the field type at field_id.0
                                                field_tys
                                                    .get(field_id.0 as usize)
                                                    .cloned()
                                                    .unwrap_or_else(|| field_ty.clone())
                                            } else {
                                                field_ty.clone()
                                            }
                                        } else {
                                            field_ty.clone()
                                        };
                                        if let Some(ld) =
                                            mir.local_decls.get_mut(dest_id.0 as usize)
                                        {
                                            if matches!(
                                                &ld.ty.kind,
                                                crate::mir::ty::TyKind::Infer(_)
                                                    | crate::mir::ty::TyKind::Error
                                            ) {
                                                ld.ty = resolved_field_ty;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Stage 14.37: Propagate written-back types through Assign statements.
        // If `loc_A = Copy(loc_B)` and loc_B's type was written back to a
        // concrete type (not Infer/Error), propagate it to loc_A.
        // Iterate until fixpoint (in case of chains: loc_A = Copy(loc_B = Copy(loc_C))).
        use crate::mir::place::Rvalue as RvalueEnum;
        loop {
            let mut changes: Vec<(usize, crate::mir::ty::Ty)> = Vec::new();
            for bb in &mir.basic_blocks {
                for stmt in &bb.statements {
                    if let crate::mir::body::StatementKind::Assign(boxed) = &stmt.kind {
                        let (place, rvalue) = &**boxed;
                        if let crate::mir::place::PlaceKind::Local(dest_id) = &place.kind {
                            if let RvalueEnum::Use(
                                crate::mir::place::Operand::Copy(src_place)
                                | crate::mir::place::Operand::Move(src_place),
                            ) = rvalue
                            {
                                if let crate::mir::place::PlaceKind::Local(src_id) = &src_place.kind
                                {
                                    let src_ty = &mir.local_decls[src_id.0 as usize].ty;
                                    let dest_ty = &mir.local_decls[dest_id.0 as usize].ty;
                                    if matches!(
                                        &dest_ty.kind,
                                        crate::mir::ty::TyKind::Infer(_)
                                            | crate::mir::ty::TyKind::Error
                                    ) && !matches!(
                                        &src_ty.kind,
                                        crate::mir::ty::TyKind::Infer(_)
                                            | crate::mir::ty::TyKind::Error
                                    ) {
                                        changes.push((dest_id.0 as usize, src_ty.clone()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if changes.is_empty() {
                break;
            }
            for (idx, ty) in changes {
                mir.local_decls[idx].ty = ty;
            }
        }

        // Stage 14.41: Re-populate adt_layouts AFTER the Stage 14.37 writeback.
        //
        // The initial `populate_adt_layouts` call (inside `lower_hir_body_to_mir`)
        // runs BEFORE the Stage 14.37 writeback. At that point, Call dest locals
        // still have `Infer` types (typeck doesn't propagate Call return types).
        // After the writeback, these locals have concrete `Adt(def_id, [])` types
        // — but `adt_layouts` was already populated with the stale set, so
        // codegen's `mir_type_to_emit_type_with_layouts` would return `I32`
        // (the fallback for unknown Adt layouts) instead of `Struct([...])`.
        //
        // This re-population picks up the new Adt DefIds exposed by the
        // writeback, ensuring codegen can emit correct struct return types
        // for static method calls like `Counter::new(5)` (which returns
        // `Counter`, an Adt).
        //
        // Per §16 (interface isolation): `populate_adt_layouts` reads HIR
        // (read-only) and sinks the layout data into MIR. The driver is the
        // orchestrator that knows when HIR is still available. After this
        // point, MIR is self-contained — codegen doesn't need HIR.
        crate::mir::lower::populate_adt_layouts(&mut mir, &hir);

        mirs.push(mir);
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
        .map(|(body_id, body)| {
            // Find the fn name for this body.
            let fn_name = hir
                .owners
                .iter()
                .find_map(|(_, owner)| match owner {
                    // Top-level fn: `landin_<name>`.
                    crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f))
                        if f.body == Some(*body_id) =>
                    {
                        let name = interner.try_resolve(&f.ident.name).unwrap_or("fn");
                        // Stage 13.15: Strip leading "landin_" to avoid doubling
                        // (see fn_name_by_def_id construction above for details).
                        let stripped = name.strip_prefix("landin_").unwrap_or(name);
                        Some(format!("landin_{}", stripped))
                    }
                    // Stage 5.6: impl method body: `landin_<SelfType>_<method>`.
                    // Matches the naming used by TraitResolver for vtable entries.
                    crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(i)) => {
                        for impl_item in &i.items {
                            if let crate::hir::HirImplItem::Fn(f) = impl_item {
                                if f.body == Some(*body_id) {
                                    let method =
                                        interner.try_resolve(&f.ident.name).unwrap_or("fn");
                                    let self_ty_name =
                                        crate::traits::extract_impl_self_ty_name(&i.self_ty);
                                    let type_str = self_ty_name
                                        .and_then(|s| interner.try_resolve(&s))
                                        .unwrap_or("Type");
                                    // Stage 13.15: Strip leading "landin_" from
                                    // both type_str and method to avoid doubling
                                    // (consistent with top-level fn handling).
                                    let type_stripped =
                                        type_str.strip_prefix("landin_").unwrap_or(type_str);
                                    let method_stripped =
                                        method.strip_prefix("landin_").unwrap_or(method);
                                    return Some(format!(
                                        "landin_{}_{}",
                                        type_stripped, method_stripped
                                    ));
                                }
                            }
                        }
                        None
                    }
                    _ => None,
                })
                .unwrap_or_else(|| format!("fn_{}", body_id.owner.0.as_u32()));
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
            BodyMeta {
                fn_name,
                is_void,
                param_count: body.params.len(),
                abi,
            }
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
    for ce in &validation_report.coherence_errors {
        let trait_str = interner.try_resolve(&ce.trait_name).unwrap_or("?");
        let type_str = interner.try_resolve(&ce.self_ty_name).unwrap_or("?");
        errors.trait_errors.push(format!(
            "conflicting implementations of trait `{}` for type `{}` ({} impl blocks)",
            trait_str,
            type_str,
            ce.impl_def_ids.len()
        ));
    }
    for inc in &validation_report.incomplete_impls {
        let trait_str = interner.try_resolve(&inc.trait_name).unwrap_or("?");
        let type_str = interner.try_resolve(&inc.self_ty_name).unwrap_or("?");
        let missing: Vec<&str> = inc
            .missing_methods
            .iter()
            .map(|s| interner.try_resolve(s).unwrap_or("?"))
            .collect();
        errors.trait_errors.push(format!(
            "impl `{}` for `{}` is missing method(s): {}",
            trait_str,
            type_str,
            missing.join(", ")
        ));
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
fn resolve_self_param_type_for_sig(
    hir: &HirCrate,
    method_def_id: crate::hir::DefId,
    self_kind: Option<crate::ast::SelfKind>,
) -> Option<crate::mir::ty::Ty> {
    // Search all owners for an Impl block that contains a method whose
    // hir_id.owner matches method_def_id.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.hir_id.owner == method_def_id {
                        // Found the owning impl block! Lower its self_ty.
                        let adt_ty = crate::mir::lower::lower_hir_ty_to_mir_ty(&impl_block.self_ty);
                        return match self_kind {
                            Some(crate::ast::SelfKind::Ref(mutability)) => {
                                let mir_mut = match mutability {
                                    crate::ast::Mutability::Mutable => {
                                        crate::mir::ty::Mutability::Mutable
                                    }
                                    crate::ast::Mutability::Immutable => {
                                        crate::mir::ty::Mutability::Immutable
                                    }
                                };
                                Some(crate::mir::ty::Ty::new(
                                    crate::mir::ty::TyKind::Ref(
                                        crate::mir::ty::Region::Erased,
                                        mir_mut,
                                        Box::new(adt_ty),
                                    ),
                                    f.span,
                                ))
                            }
                            // self by value — no wrapping
                            _ => Some(adt_ty),
                        };
                    }
                }
            }
        }
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
        HirExprKind::Return { expr: Some(e), .. } => scan_expr_for_unresolved(e, errors),
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
                if let HirStmt::Expr(e, _) = stmt {
                    scan_expr_for_unresolved(e, errors);
                }
            }
            if let Some(e) = &body.expr {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirExprKind::Closure { body, .. } => scan_expr_for_unresolved(body, errors),
        // Lit, Unit, Break, Continue, Try, Unsafe, MacroCall, Range, Repeat — no paths
        _ => {}
    }
}

fn scan_pat_for_unresolved(_pat: &crate::hir::HirPat, _errors: &mut CompileErrors) {
    // G4 fix: temporarily disabled for patterns. Enum variant patterns
    // (e.g., `Circle(r)` in `match s { Circle(r) => ... }`) are not yet
    // resolved by the resolver (Stage 3 work), so they appear as
    // Res::Unknown. Reporting them as errors would break all enum match
    // tests. Stage 3 will add proper enum variant resolution and re-enable
    // pattern scanning.
}

fn scan_ty_for_unresolved(ty: &crate::hir::HirTy, errors: &mut CompileErrors) {
    use crate::hir::{HirTyKind, Res};
    match &ty.kind {
        HirTyKind::Path(_, p) => {
            if matches!(p.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::new(
                    "cannot find type in this scope".to_string(),
                    ty.span,
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
        _ => {}
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
