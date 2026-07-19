# Stage 3 Development Log

> **Author**: redskaber
> **Date**: 2026-07-19
> **Version**: v0.7.3
> **Status**: Active

## Sub-stages

### Stage 3.1 — MVP (v0.5.1)
- Created `src/codegen/mod.rs` with direct string-based codegen
- Supports: function definition, return, i32 constants, binary ops
- 13 codegen tests

### Stage 3.2 — Variables + Control Flow + Calls (v0.5.2)
- Added alloca/store/load for variable allocation
- SwitchInt → br i1 (bool conditionals for if/while)
- Goto → br label
- Call → call instruction
- Basic block labels (bb0, bb1, ...)

### Stage 3.3 — Comparisons + Borrow/Deref (v0.6.0)
- Comparison ops (Eq/Ne/Lt/Le/Gt/Ge) → icmp + zext
- Rvalue::Ref → alloca pointer
- Projection::Deref → double load
- Call resolution from FnDef type in local's ty

### Stage 3.4 — Emitter Trait Refactor (v0.6.0)
- Introduced Emitter trait (backend-agnostic)
- TextEmitter implements Emitter for .ll text
- Translation layer walks MIR and calls Emitter methods
- Future InkwellEmitter only needs to implement Emitter trait

### Stage 3.5 — Parameter Passing (v0.7.0)
- HIR body.params.len() determines param count
- Each param gets LLVM arg: %arg0, %arg1, ...
- Params stored to alloca slots at function entry
- Call instruction emits typed args

### Stage 3.6 — Match Switch + Float (v0.7.1)
- LLVM switch instruction for match on integers
- Float constant handling (f64)
- Process v3.5: Documentation sync rules (§11)

### Stage 3.7 — Cast + Author (v0.7.2)
- Rvalue::Cast → proper LLVM cast instructions
- All cast types: sext/zext/trunc/sitofp/fptosi/fpext/fptrunc/bitcast
- Author "redskaber" added to all documents
- Process v3.6: Author标注

### Stage 3.8 — Doc Reorganization (v0.7.3)
- Process v3.7: Document organization rules (§12)
- Reorganized docs/ into agent-team/ + develop/v0/stage-N/ + lang-design/
- Created lang-design design documents
- Created agent-team role definitions

## Test Progression

| Version | Tests | New |
|---------|-------|-----|
| v0.5.1 | 686 | +13 codegen |
| v0.5.2 | 686 | 0 (refactor) |
| v0.6.0 | 694 | +8 (comparisons, borrow, call) |
| v0.7.0 | 699 | +5 (params, calls) |
| v0.7.1 | 706 | +7 (match, float, complex) |
| v0.7.2 | 709 | +3 (cast, bool return) |
| v0.7.3 | 709 | 0 (doc reorg) |
