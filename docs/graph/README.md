# Pipeline Diagrams Index

> **Date**: 2026-08-26
> **Version**: v0.493.0 (Stage 18.318)

## Directory Structure

| Directory | Files | Description |
|-----------|-------|-------------|
| `codegen/` | 5 | Codegen module architecture, emitter trait, data flow, backend comparison |
| `closure/` | 1 | Closure data flow (HIR → MIR → Codegen) |
| `pipeline/` | 1 | End-to-end compiler pipeline overview |
| `error-system/` | 1 | Error types, error flow, error codes, reporting |
| `type-system/` | 1 | Type checking + borrow checking data flow |
| `trait-system/` | 1 | Static + dynamic dispatch, vtable layout, Copy detection |

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

## Stage 16.47 Update

Added 3 new diagram directories:
- `error-system/` — Error system data flow
- `type-system/` — Type system data flow (typeck + borrowck)
- `trait-system/` — Trait system data flow (static + dynamic dispatch)

Total graph diagrams: 11 (was 8)
