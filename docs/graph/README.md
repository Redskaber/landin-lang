# Pipeline Diagrams Index

> **Date**: 2026-08-31
> **Version**: v0.557.0 (Stage 30.22)
> **Stage**: 30.22 (graph docs gap closure)

## Directory Structure

| Directory | Files | Description |
|-----------|-------|-------------|
| `codegen/` | 5 | Codegen module architecture, emitter trait, data flow, backend comparison |
| `closure/` | 1 | Closure data flow (HIR → MIR → Codegen) |
| `pipeline/` | 1 | End-to-end compiler pipeline overview |
| `error-system/` | 1 | Error types, error flow, error codes, reporting |
| `type-system/` | 1 | Type checking + borrow checking data flow |
| `trait-system/` | 1 | Static + dynamic dispatch, vtable layout, Copy detection |
| `lexer/` | 1 | Lexer data flow (source → tokens) |
| `parser/` | 1 | Parser data flow (tokens → AST) |
| `hir/` | 1 | HIR data flow (AST → HIR) |
| `mir/` | 1 | MIR data flow (HIR → MIR CFG) |
| `typeck/` | 1 | Typeck data flow (MIR + tables → typed MIR) |
| `borrowck/` | 1 | Borrowck data flow (MIR + NLL → borrow-checked MIR) |
| `traits/` | 1 | Traits data flow (HIR → TraitResolver + Solver) |
| `driver/` | 1 | Driver data flow (orchestrator of all passes) |
| `resolve/` | 1 | Resolve data flow (HIR with Res::Unknown → HIR with Res::*) |

## Complete Diagram List

| File | Description |
|------|-------------|
| `codegen/README.md` | Codegen graph directory index |
| `codegen/architecture.md` | Codegen module architecture (final post-refactoring) |
| `codegen/emitter-trait.md` | Emitter trait hierarchy (39 methods, 3 doc groups) |
| `codegen/data-flow.md` | Unified pipeline data flow (MIR → LLVM IR) |
| `codegen/backend-comparison.md` | TextEmitter vs LLVMSysEmitter comparison |
| `closure/data-flow.md` | Closure data flow (HIR → MIR → Codegen) |
| `pipeline/overview.md` | End-to-end compiler pipeline |
| `error-system/data-flow.md` | Error types, flow, codes, reporting |
| `type-system/data-flow.md` | Typeck + borrowck + iterative typeck data flow |
| `trait-system/data-flow.md` | Static/dynamic dispatch, vtable, Copy detection, DefId lookup |
| `lexer/data-flow.md` | Lexer data flow (source → tokens via hand-written recursive scanner) |
| `parser/data-flow.md` | Parser data flow (tokens → AST via recursive descent + Pratt) |
| `hir/data-flow.md` | HIR data flow (AST → HIR with HirId/Res/InferTy placeholders) |
| `mir/data-flow.md` | MIR data flow (HIR Body → MirBody CFG of basic blocks) |
| `typeck/data-flow.md` | Typeck data flow (MIR + tables → typed MIR via unification) |
| `borrowck/data-flow.md` | Borrowck data flow (MIR + TraitResolver → NLL liveness + move tracking) |
| `traits/data-flow.md` | Traits data flow (HIR → TraitResolver + v0.5 trait solver) |
| `driver/data-flow.md` | Driver data flow (orchestrates lexer → parser → HIR → resolve → MIR → typeck → borrowck → codegen) |
| `resolve/data-flow.md` | Resolve data flow (HIR Res::Unknown → Res::Def/Local/PrimTy/SelfTy) |

## Stage 16.47 Update

Added 3 new diagram directories:
- `error-system/` — Error system data flow
- `type-system/` — Type system data flow (typeck + borrowck)
- `trait-system/` — Trait system data flow (static + dynamic dispatch)

Total graph diagrams: 11 (was 8)

## Stage 30.22 Update (graph docs gap closure)

Added 9 new per-module `data-flow.md` documents, each 100-160 lines,
covering the core compilation passes:

- `lexer/` — Hand-written recursive scanner (6 sub-modules: ident, number,
  string, operators, keywords, raw strings). Entry point: `tokenize`.
- `parser/` — Recursive descent + Pratt parser (7 sibling sub-modules +
  macro_expand engine). Entry point: `parse_crate`.
- `hir/` — AST → HIR lowering with HirId/DefId/Res/InferTy placeholders.
  Entry point: `lower_crate`.
- `mir/` — HIR Body → MirBody CFG (16 sub-modules for lowering concerns).
  Entry point: `lower_hir_body_to_mir_full`.
- `typeck/` — MIR + pre-computed tables → typed MIR via unification.
  Canonical entry: `check_mir_body_with_tables` (§16-compliant, Stage 18.60).
- `borrowck/` — MIR + TraitResolver → NLL liveness + move tracking +
  borrow set. Canonical entry: `check_mir_body_with_dataflow`.
- `traits/` — HIR → TraitResolver + v0.5 trait solver (HrtbBound,
  assoc_type_bindings for Stage 30.10/30.12).
- `driver/` — Top-level orchestrator with 10-field `CompileErrors`,
  4-way split (Stage 18.134 / 18.138 J1-J6 §13.4), `module_loader`
  for multi-file (Stage 18.152).
- `resolve/` — HIR Res::Unknown → Res::* (5 passes per §6.2,
  impl_method_index for `V::new` method resolution — Stage 14.41).

All 9 documents follow the closure/data-flow.md template:
- Title + meta block (Date / Version / Stage)
- Module Overview (1-2 paragraphs)
- ASCII art Data Flow Diagram
- Key Data Structures (3-5 items with field descriptions)
- Dependencies (upstream inputs + downstream consumers)
- Stage Boundaries (§16 interface isolation, §13.4 / §14.4 file splits)

Total graph diagrams: 20 (was 11)
