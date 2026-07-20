# Landin Compiler — Release Notes

**Author**: redskaber
**Current version**: v0.8.6
**Date**: 2026-07-20
**Test count**: 796 tests passing, 0 warnings, fmt + clippy clean

---

## v0.8.6 — Stage 3.21–3.37 (typed codegen + runtime checks + literals + ADT structs + field type resolution + field mutation + 6 gate review rounds)

### Stage 3.21 — Typed aggregate codegen
- `EmitType` now carries full structure: `Struct(Vec<EmitType>)`, `Array(Box<EmitType>, u64)`,
  `Ptr(Box<EmitType>)` (was hardcoded `{ i32 }` / `[10 x i32]` / opaque `i32*`).
- 10 new tests.

### Stage 3.22 — Block-scoped local value cache
- **Bug fix**: `if x > 0 { 1 } else { 2 }` previously returned `2` regardless of `x`.
- 6 new tests.

### Stage 3.24 — Real overflow checks
- **Bug fix**: overflow checks never fired. `a + b` silently wrapped (UB).
- 8 new tests.

### Stage 3.25 — Real div-by-zero checks
- **Bug fix**: `a / 0` invoked LLVM `sdiv` — UB.
- 6 new tests.

### Stage 3.27 — String literal codegen
- **Bug fix**: `ConstVal::Str` hardcoded to emit `"0"` (null pointer).
- 13 new tests.

### Stage 3.28 — Byte string literal codegen
- **Bug fix**: `b"..."` literals and `u8`/`i8` types fell through to `I32`.
- 9 new tests.

### Stage 3.30 — ADT/struct codegen + §15/§16 process principles
- **Process v3.10 + v3.11**: added §15 (最优 > 最小) and §16 (阶段间接口隔离).
- **3 root-cause bugs fixed**: tuple struct ctor as Call, named struct type lost,
  field index hardcoded 0.
- **§16 compliance**: `AggregateKind::Adt` extended with `field_tys: Vec<Ty>`.
- 13 new tests.

### Stage 3.32 — L-DEBT-2 fix: field type resolution through projections
- **Bug fix**: `p.1` where field 1 is `i64` loaded as `i32` (silent truncation).
- **Fix** (per §15): typeck `infer_rvalue` handles `AggregateKind::Adt`; new
  Phase 3.5 `writeback_field_types`; MIR lower `resolve_field_index` fallback scan.
- 6 new tests.

### Stage 3.34 — L-MUT-1 fix: field mutation MIR lower
- **Bug fix**: `a.v = 42` didn't mutate the struct (silently dropped).
- **Root cause**: MIR lower's `HirExprKind::Assign` only handled `Path` LHS.
- **Fix** (per §15): new `lower_expr_to_lvalue` function handles all LHS shapes
  (Path, Field, Index, Deref). `HirExprKind::Assign` uses it generically.
- 8 new tests.

### Gate Reviews Round 1-6
- R1: 38-case audit, 5/5 APPROVED
- R2: 43-case audit, 5/5 APPROVED
- R3: 43-case audit, 5/5 APPROVED
- R4: 37-case audit, 5/5 APPROVED
- R5: 30-case audit, 5/5 APPROVED
- R6: 30-case audit, 5/5 APPROVED
- §9.3.3 CONVERGED: 6 consecutive rounds with 0 new issues
- L2 (struct codegen) + L4 (string literals) + L6 (overflow) + L7 (div-by-zero)
  + L12 (u8/i8 type) + L-DEBT-2 (field type resolution) + L-MUT-1 (field mutation) CLOSED.
- Remaining: L1 PHI, L3 closures, L5 traits, L8 lli, L9 i128, L10 float-bitwise,
  L11 shift-count, L13 fat pointers, L14 i16, L15 str-as-arg, L-ENUM enum variants,
  L-PIPE-1 HIR lookup for Adt storage, L-DEBT-3 field type propagation through arithmetic.

### Changed
- `Cargo.toml`: v0.8.5 → v0.8.6
- `src/codegen/{emitter.rs, text_emitter.rs, mod.rs}`: typed codegen + string globals + ADT/struct codegen + `hir_ty_to_emit_type`
- `src/mir/{body.rs, lower/mod.rs, lvalue.rs}`: AssertMessage extended, AggregateKind::Adt field_tys, resolve_field_index/resolve_field_type/resolve_adt_field_tys, HirTyKind::Path → TyKind::Adt, lower_expr_to_lvalue
- `src/typeck/checker.rs`: AggregateKind::Adt handling in infer_rvalue, Phase 3.5 writeback_field_types, check_mir_body_with_hir
- `src/hir/kinds.rs`: `Res::Def(DefId, DefKind)`
- `src/resolve/resolver.rs`: populates `DefKind`
- `src/parser/parser.rs`: `&mut Rodeo` + tuple field index interning
- `src/driver.rs`: passes `&mut interner` + `&hir` to MIR Lower + `check_mir_body_with_hir`
- `tests/codegen_tests.rs`: +79 tests (total 115)
- `examples/stage3_gate_audit{,_r2..r6}.rs`: 6 audit tools
- `docs/develop/v0/stage-3/{dev-log.md, gate-review-round1..6.md}`
- `docs/stage-committee-process.md`: §15 + §16

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
|---------|--------|
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
| v3.8 | (Pending) Stage 3 gate review convergence rule for codegen |
