# Landin Compiler — Release Notes

**Author**: redskaber
**Current version**: v0.7.4
**Date**: 2026-07-19
**Test count**: 709 tests passing, 0 warnings, fmt + clippy clean

---

## v0.7.4 — Stage 3.9: Imported user-provided documentation (process v3.7)

### Added — agent-team/ (12 new documents)

- `00-requirement-history.md` — Requirements evolution history
- `01-agent-team-overview.md` — Agent team structure overview
- `02-agent-roles-detail.md` — Detailed role definitions (25 roles)
- `03-collaboration-workflow.md` — Inter-agent collaboration workflow
- `04-agent-skills.md` — Agent skill definitions
- `05-meeting-and-decision-log.md` — Meeting records and decisions
- `06-risk-register.md` — Project risk tracking
- `07-team-charter.md` — Team charter and principles
- `08-agent-lifecycle.md` — Agent lifecycle management
- `09-runtime-protocol.md` — Runtime communication protocol
- `10-modernization-roadmap.md` — Modernization roadmap
- `README.md` — Agent team index

### Added — lang-design/ (20 new documents)

- `01-language-specification.md` — Full language specification
- `02-grammar.md` — Grammar definition (EBNF)
- `03-type-system.md` — Type system design
- `04-ownership-borrowing.md` — Ownership and borrowing design
- `05-ast.md` — AST structure design
- `06-mir.md` — MIR design
- `07-codegen.md` — LLVM codegen design (replaces our 08-codegen.md)
- `08-bootstrap-strategy.md` — Self-hosting strategy
- `09-stdlib.md` — Standard library design
- `10-toolchain.md` — Toolchain design
- `11-testing.md` — Testing strategy
- `12-roadmap.md` — Project roadmap
- `13-stage1-feature-whitelist.md` — Stage 1 feature whitelist
- `14-soundness-considerations.md` — Soundness analysis
- `15-attributes.md` — Attribute system design
- `16-diagnostics.md` — Diagnostic system design
- `17-conformance-suite.md` — Conformance test suite
- `18-glossary.md` — Glossary of terms
- `19-project-meta.md` — Project metadata
- `CHANGELOG.md` — Language design changelog
- `FREEZE-REPORT.md` — Design freeze report
- `README.md` — Language design index

### Changed

- Consolidated uploaded docs into our v0/stage-N structure
- Removed duplicate flat docs/develop/ files (kept v0/stage-N/ versions)
- Process docs restored to v3.7

### Document count

- docs/agent-team/: 12 files (was 2)
- docs/lang-design/: 22 files (was 2)
- docs/develop/v0/stage-N/: 18 files (unchanged)
- Total docs: 56 files (was 25)

---

## v0.7.3 — Stage 3.8: Doc reorganization (process v3.7)

### Added

- Process v3.7 §12: Document organization structure rules

---

## v0.7.2 — Stage 3.7: Author + cast codegen (process v3.6)

### Added

- Author "redskaber" added to all project documents
- Cast codegen (sext/zext/trunc/sitofp/fptosi/fpext/fptrunc)

---

## v0.7.1 — Stage 3.5: Parameter passing + doc sync (process v3.5)

### Added

- Parameter passing: `fn add(a: i32, b: i32) -> i32 { a + b }` generates
  `define i32 @fn_0(i32 %arg0, i32 %arg1)` with params stored to alloca slots

---

## v0.5.0 — Stage 3.1-3.4: LLVM codegen MVP

### Added

- `src/codegen/` module with Emitter trait + TextEmitter
- LLVM IR text output (.ll)

---

## v0.4.9 — Stage 0-2 OFFICIAL FINAL

### Summary

- Stage 0 (lexer/parser): 245 tests, 0 issues
- Stage 1 (HIR/resolve): 451 tests, 0 issues  
- Stage 2 (MIR/typeck/borrowck): 673 tests, 0 issues
- 6 rounds of phase gate review, 233 cumulative audit cases

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
| v3.6 | Author标注规则 |
| v3.7 | 文档组织结构规则 (§12) |
