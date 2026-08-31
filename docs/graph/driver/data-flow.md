# Driver Data Flow (orchestrator of all passes)

> **Date**: 2026-08-31
> **Version**: v0.557.0
> **Stage**: 30.22 (graph docs gap closure)

## Module Overview

The driver is the top-level orchestrator that wires together every
compilation pass: lexer → parser → HIR lower → resolve → MIR lower →
typeck → drop elaboration → borrowck → codegen. It owns the
`CompileErrors` aggregate (10 fields: lex / parse / lower / resolve /
typeck / borrowck / trait_errors / macro_errors / codegen /
module_load), the `DriverState`, the pre-computation phase that builds
`FieldTyTable`, `FnSigTable`, and `TraitResolver` (so typeck / borrowck
/ codegen never read HIR), and the `&mut Rodeo` interner that lives
across the whole compilation.

Stage 18.134 §13.4 J1-J6 split the driver into 4 sibling sub-modules:
`driver_scan.rs` (scan_for_unresolved_paths),
`driver_codegen_prep.rs` (codegen prep + DynTraitMIRPlan build),
`driver_validations.rs` (owner_return_ty + visibility checks),
`projection_resolver.rs` (associated type projection resolution).
Stage 18.152 (TD-SINGLE-FILE Phase 1) added `module_loader.rs` for
multi-file module loading. The canonical entry point is
`compile(source, &mut Rodeo) -> CompileResult`.

## Data Flow Diagram

```
source: &str (or Vec<PathBuf> for multi-file)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  1. lexer::tokenize(src, &mut Rodeo)  →  (Vec<Token>, Vec<LexError>) │
│     errors → CompileErrors.lex (FATAL)                       │
└─────────────┬────────────────────────────────────────────────┘
              │ (if !has_fatal)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  2. parser::parse_crate(tokens, &mut Rodeo) → (Crate, Vec<ParseError>) │
│     errors → CompileErrors.parse (FATAL)                     │
│     macro_errors → CompileErrors.macro_errors                │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│  3. module_loader::ModuleLoader (Stage 18.152, multi-file)   │
│     resolves mod foo; → load foo.lin                         │
│     errors → CompileErrors.module_load                        │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│  4. hir::lower::lower_crate(&ast, &Rodeo) → (HirCrate, Vec<LowerError>) │
│     errors → CompileErrors.lower (non-fatal, Stage 18.75 P0-1)│
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│  5. resolve::resolve_crate(&mut hir, &Rodeo) → Vec<ResolveError> │
│     mutates HirPath.res: Res::Unknown → Res::Def/Local/...   │
│     errors → CompileErrors.resolve (non-fatal)               │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│  6. traits::TraitResolver::collect(&hir, &interner)          │
│     Pre-compute TraitInfo, ImplInfo, Vtable, derived_copy_types│
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│  7. driver_codegen_prep.rs (Stage 18.138 extraction)         │
│     - Build FieldTyTable from HIR (struct/enum field types)   │
│     - Build FnSigTable from HIR (all fn signatures)          │
│     - Build DynTraitMIRPlan from TraitResolver               │
│     - Build SharedAdtLayouts (Arc<AdtLayouts>)               │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│  8. For each hir::Body:                                       │
│     mir::lower::lower_hir_body_to_mir_full_with_dyn_trait_plan │
│       → MirBody + UnificationTable                            │
│     typeck::check_mir_body_with_tables(mir, field_tys, fn_sigs)│
│       → TypeckResults + modified MirBody + TypeErrors         │
│     mir::drop_elaboration::elaborate_drops(&mut mir)          │
│     borrowck::check_mir_body_with_dataflow(&mir, resolver)   │
│       → Vec<BorrowError>                                      │
└─────────────┬────────────────────────────────────────────────┘
              │ (post-typeck writeback for tuple/field Infer)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  9. driver_writeback:                                         │
│     - Tuple literal type writeback (driver.rs)                │
│     - Field projection Copy dest writeback (driver.rs)        │
│     - detect_place_type Infer resolution (mir_translation.rs) │
│     - Iterative typeck fixpoint (re-typeck closures 4×)       │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│ 10. codegen::codegen_crate(&CompileResult)                   │
│     → String (LLVM IR text) or LLVM module                   │
│     errors → CompileErrors.codegen (non-fatal, Stage 18.75)  │
└─────────────┬────────────────────────────────────────────────┘
              │
              ▼
              → CompileResult { mirs, errors, hir, resolver, ... }
              → CLI: write to file or pipe to llc/opt/llvm-link
```

## Key Data Structures

- **`CompileErrors`** (`src/driver/mod.rs`) — 10-field aggregate:
  `lex`, `parse`, `lower`, `resolve`, `typeck`, `borrowck`,
  `trait_errors`, `macro_errors`, `codegen`, `module_load`. Has
  `is_empty()`, `total_count()`, `has_fatal()` (lex or parse non-empty),
  and `to_diagnostics_with_resolver()` (Stage 16.83).
- **`DriverState`** — Holds intermediate compilation artifacts:
  `Option<HirCrate>`, `Option<MirBody>` (per fn), `Option<TypeckResults>`,
  `Option<TraitResolver>`, the `Rodeo` interner, accumulated errors.
- **`CompileResult`** — Final output: `{ mirs: HashMap<DefId, MirBody>,
  errors: CompileErrors, hir: HirCrate, resolver: TraitResolver,
  fn_name_by_def_id, vtable layouts, dyn_trait_plan, ... }`.
- **`ModuleLoader`** (`src/driver/module_loader.rs`) — Stage 18.152
  multi-file module loader; resolves `mod foo;` declarations to
  file paths, loads + parses each, merges ASTs.
- **`DynTraitMIRPlan`** (built by `driver_codegen_prep.rs` from
  `TraitResolver`) — Consumed by `mir::lower` and `codegen::pipeline`
  to emit vtables + dynptr globals (see `trait-system/data-flow.md`).

## Dependencies

**Upstream inputs:**
- Source text (`&str`) or module file list (`Vec<PathBuf>`) — from CLI.
- `&mut Rodeo` — the interner, owned by the driver for the compilation.

**Downstream consumers:**
- `src/bin/landinc.rs` — CLI entry; calls `compile`, writes output.
- `src/bin/main.rs` — alternate entry for testing / REPL.
- `src/driver/driver_tests.rs` — integration tests.

## Stage Boundaries

Per §16 (interface isolation), the driver is the ONLY place where HIR
is read by later passes' pre-computation phases: it builds
`FieldTyTable`, `FnSigTable`, `SharedAdtLayouts`, `TraitResolver`,
and `DynTraitMIRPlan` once, then hands them as data contracts to
typeck / borrowck / codegen — none of which read HIR directly. Stage
18.60 closed the long-standing §16 violation in typeck (removed
`check_crate` and `check_mir_body_with_hir`). Stage 18.75 P0-1 added
`lower` field to CompileErrors (previously silently discarded). The
4-way driver split (Stage 18.134 / 18.138) follows §13.4 J1-J6
single-responsibility. The driver sits at pipeline position 0 (entry)
and orchestrates positions 1-8. Error policy: lex + parse are fatal
(`has_fatal()` short-circuits the pipeline); all other error
categories are non-fatal (compilation continues, MIR/codegen produced
with placeholder nodes; user sees all errors at once).
