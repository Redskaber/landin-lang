# Landin Compiler — Release Notes

**Author**: redskaber
**Current version**: v0.8.6
**Date**: 2026-07-20
**Test count**: 739 tests passing, 0 warnings, fmt + clippy clean

---

## v0.8.6 — Stage 3.21–3.26 (typed codegen + real runtime checks + 2 gate review rounds)

### Stage 3.21 — Typed aggregate codegen
- `EmitType` now carries full structure: `Struct(Vec<EmitType>)`, `Array(Box<EmitType>, u64)`,
  `Ptr(Box<EmitType>)` (was hardcoded `{ i32 }` / `[10 x i32]` / opaque `i32*`).
- `emit_type_to_llvm_str` returns `String` (was `&'static str`).
- `emit_gep_field` / `emit_gep_index` now take the actual struct/array type.
- `emit_insertvalue` now takes `val_ty: &EmitType` for the inserted value.
- `emit_call` now takes `args: &[(EmitType, &EmitValue)]` — typed call args
  (was hardcoded `i32` for every arg).
- 10 new tests.

### Stage 3.22 — Block-scoped local value cache
- **Bug fix**: `if x > 0 { 1 } else { 2 }` previously returned `2` regardless of `x`,
  because `TextEmitter::locals` cached the most-recent assignment across block boundaries.
- **Fix**: `emit_block` now clears `self.locals` at each block boundary. `local_ptrs`
  (alloca handles) persist. Within-block constant shortcut still works.
- 6 new tests verifying if-else / match / while merge correctness.

### Stage 3.24 — Real overflow checks
- **Bug fix**: `Assert` for overflow used `cond = Bool(true)` placeholder — overflow
  checks never fired. `a + b` silently wrapped on overflow (UB in safe Landin).
- **Fix**: Extended `AssertMessage::Overflow` to carry lhs/rhs operands.
  Codegen emits `llvm.{sadd,ssub,smul}.with.overflow.{i32,i64}`, extracts the i1
  overflow flag via `extractvalue`, inverts with `xor i1 ..., -1`, and branches
  to a panic block on overflow.
- 8 new tests.

### Stage 3.25 — Real div-by-zero checks
- **Bug fix**: Div/Rem had no divisor==0 check. `a / 0` invoked LLVM `sdiv` —
  undefined behavior on zero divisor.
- **Fix**: Extended `AssertMessage::DivisionByZero` to carry the divisor operand.
  Codegen emits `icmp eq <divisor>, 0` and branches to a panic block on true.
  MIR lower now emits `DivisionByZero(rhs)` for Div/Rem (was wrongly emitting
  `Overflow(op)` which fell back to "no check").
- 6 new tests.

### Stage 3.23 + 3.26 — Gate Reviews Round 1 + Round 2
- R1: 38-case audit (`examples/stage3_gate_audit.rs`), 5/5 APPROVED
- R2: 43-case audit (`examples/stage3_gate_audit_r2.rs`), 5/5 APPROVED
- §9.3.3 CONVERGED: 2 consecutive rounds with 0 new issues
- L6 (overflow) + L7 (div-by-zero) CLOSED; remaining items are optimizations
  (L1 PHI, L10 float-bitwise) or new features (L2 ADT, L3 closures, L4 strings, L5 traits)

### Changed
- `Cargo.toml`: v0.8.5 → v0.8.6
- `src/codegen/emitter.rs`: EmitType refactor + new `emit_checked_binop` trait method
- `src/codegen/text_emitter.rs`: updated impls + `emit_checked_binop` + block-scoped cache
- `src/codegen/mod.rs`: real overflow + div-by-zero check emission
- `src/mir/body.rs`: `AssertMessage::Overflow(BinOp, Operand, Operand)` + `DivisionByZero(Operand)`
- `src/mir/lower/mod.rs`: `emit_overflow_assert` passes lhs/rhs; new `emit_div_by_zero_assert`
- `tests/codegen_tests.rs`: +30 tests (total 66)
- `tests/deep_inspection.rs`, `tests/integration_stage2_4c.rs`, `examples/round5_deep.rs`: updated pattern matches
- `examples/stage3_gate_audit.rs`, `examples/stage3_gate_audit_r2.rs`: new audit tools
- `docs/develop/v0/stage-3/{dev-log.md, gate-review-round1.md, gate-review-round2.md}`: full reports

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
