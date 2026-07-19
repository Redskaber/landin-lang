# Landin Compiler — Release Notes

**Author**: redskaber
**Current version**: v0.7.2
**Date**: 2026-07-19
**Test count**: 706+ tests passing, 0 warnings, fmt + clippy clean

---

## v0.7.2 — Stage 3.7: Author + codegen improvements (process v3.6)

### Added

- Author "redskaber" added to all project documents (Cargo.toml, README, RELEASE_NOTES, process docs)
- Process v3.6: Author标注规则

### Changed

- Cargo.toml: added `authors = ["redskaber"]`, version bumped to 0.7.2

---

## v0.7.1 — Stage 3.5: Parameter passing + doc sync (process v3.5)

### Added

- Parameter passing: `fn add(a: i32, b: i32) -> i32 { a + b }` generates
  `define i32 @fn_0(i32 %arg0, i32 %arg1)` with params stored to alloca slots
- Call with typed args: `call i32 @fn_0(i32 3, i32 4)`
- Process v3.5 §11: Documentation sync rules (Cargo.toml, README, docs/)
- Updated Cargo.toml to v0.7.1
- Updated README with codegen capabilities table
- Updated this RELEASE_NOTES.md

### Codegen capabilities

- Function definition with parameters
- Return values
- Arithmetic (add/sub/mul/div/rem)
- Comparison (icmp eq/ne/lt/le/gt/ge + zext)
- Unary (neg/not)
- Variables (alloca/store/load)
- Control flow (if/while → br/cond_br)
- Borrow (&x → alloca pointer)
- Deref (*r → load through pointer)
- Function calls (call with typed args)
- Recursive calls (fibonacci)

### Architecture

- `Emitter` trait: backend-agnostic codegen interface
- `TextEmitter`: .ll text output (current)
- `InkwellEmitter`: future LLVM C API backend
- Translation layer walks MIR and calls Emitter methods

---

## v0.5.0 — Stage 3.1-3.4: LLVM codegen MVP

### Added

- `src/codegen/` module with Emitter trait + TextEmitter
- LLVM IR text output (.ll)
- Function definition, return, constants, arithmetic, comparison
- Variables (alloca/store/load), control flow (br), function calls
- Borrow/deref codegen
- 26 codegen tests

---

## v0.4.9 — Stage 0-2 OFFICIAL FINAL

### Summary

- Stage 0 (lexer/parser): 245 tests, 0 issues
- Stage 1 (HIR/resolve): 451 tests, 0 issues  
- Stage 2 (MIR/typeck/borrowck): 673 tests, 0 issues
- 6 rounds of phase gate review, 233 cumulative audit cases
- Process v3.4 (收敛规则)

---

## Process version history

| Version | Change |
| --------- | -------- |
| v1.0 | Initial 5-role + voting + 4-7 rounds |
| v2.0 | Dynamic rounds + defect grading + weighted voting |
| v3.0 | Integration verification + P3 reclassification + gate review |
| v3.1 | Negative-test coverage matrix (§9.1.1) |
| v3.2 | Expanded audit requirement ≥30 cases (§9.3.1) |
| v3.3 | Previous-round-fix edge case tests (§9.3.2) |
| v3.4 | Diminishing returns rule + Stage 3 start conditions (§9.3.3) |
| v3.5 | Documentation sync rules (§11) |
