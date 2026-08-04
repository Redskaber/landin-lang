# Codegen Architecture Data Flow Diagrams

> **Author**: redskaber
> **Date**: 2026-08-04 (Stage 16.35)
> **Purpose**: Standardized codegen architecture diagrams for the Landin compiler.

## Directory Structure

- `codegen/` — Codegen module architecture diagrams
- `pipeline/` — End-to-end compiler pipeline diagrams
- `closure/` — Closure-specific data flow diagrams

## Diagram Types

1. **Module architecture diagrams** — show the module/file structure
2. **Data flow diagrams** — show how data flows through the pipeline
3. **Stage diagrams** — show the pipeline stages and their inputs/outputs
4. **Keyword diagrams** — show specific feature data flows (e.g., closure, dyn Trait)

## Notation

- `[Box]` = module/file
- `{Box}` = data structure
- `→` = data flow
- `⇒` = transformation
- `|` = alternative path
- `▼` = entry point

## Files

- `codegen/architecture.md` — Codegen module architecture (post-Stage 16.35)
- `codegen/data-flow.md` — Codegen data flow (MIR → IR)
- `codegen/emitter-trait.md` — Emitter trait hierarchy
- `pipeline/overview.md` — End-to-end compiler pipeline
- `pipeline/codegen-stage.md` — Codegen stage detail
- `closure/data-flow.md` — Closure data flow (HIR → MIR → Codegen)
