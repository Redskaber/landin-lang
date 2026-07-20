# Landin Compiler — Release Notes

**Author**: redskaber
**Current version**: v0.8.6
**Date**: 2026-07-20
**Test count**: 761 tests passing, 0 warnings, fmt + clippy clean

---

## v0.8.6 — Stage 3.21–3.29 (typed codegen + runtime checks + string literals + 3 gate review rounds)

### Stage 3.21 — Typed aggregate codegen
- `EmitType` now carries full structure: `Struct(Vec<EmitType>)`, `Array(Box<EmitType>, u64)`,
  `Ptr(Box<EmitType>)` (was hardcoded `{ i32 }` / `[10 x i32]` / opaque `i32*`).
- `emit_type_to_llvm_str` returns `String` (was `&'static str`).
- `emit_gep_field` / `emit_gep_index` now take the actual struct/array type.
- `emit_insertvalue` now takes `val_ty: &EmitType` for the inserted value.
- `emit_call` now takes `args: &[(EmitType, &EmitValue)]` — typed call args.
- 10 new tests.

### Stage 3.22 — Block-scoped local value cache
- **Bug fix**: `if x > 0 { 1 } else { 2 }` previously returned `2` regardless of `x`,
  because `TextEmitter::locals` cached the most-recent assignment across block boundaries.
- **Fix**: `emit_block` now clears `self.locals` at each block boundary.
- 6 new tests.

### Stage 3.24 — Real overflow checks
- **Bug fix**: `Assert` for overflow used `cond = Bool(true)` placeholder — overflow
  checks never fired. `a + b` silently wrapped on overflow (UB in safe Landin).
- **Fix**: Extended `AssertMessage::Overflow` to carry lhs/rhs operands.
  Codegen emits `llvm.{sadd,ssub,smul}.with.overflow.{i32,i64}`, extracts the i1
  overflow flag via `extractvalue`, inverts with `xor i1 ..., -1`, and branches
  to a panic block on overflow.
- 8 new tests.

### Stage 3.25 — Real div-by-zero checks
- **Bug fix**: Div/Rem had no divisor==0 check. `a / 0` invoked LLVM `sdiv` — UB.
- **Fix**: Extended `AssertMessage::DivisionByZero` to carry the divisor operand.
  Codegen emits `icmp eq <divisor>, 0` and branches to a panic block on true.
- 6 new tests.

### Stage 3.27 — String literal codegen
- **Bug fix**: `ConstVal::Str(sym)` hardcoded to emit `"0"` (null pointer).
  Any program using string literals produced broken IR — bytes were lost.
- **Fix**: Added `Emitter::emit_string_global(bytes)` trait method.
  `TextEmitter` accumulates + dedupes string globals, emits them at module
  end via `output_with_globals()`. Each global:
  `@.str.N = private unnamed_addr constant [M x i8] c"..."`.
  Bytes escaped: printable ASCII verbatim; everything else as `\NN` hex.
  `codegen_operand` for `ConstVal::Str`: looks up bytes via interner (now
  threaded through all codegen functions), emits global, returns GEP → i8*.
  `TyKind::Str` maps to `EmitType::ptr_to(EmitType::I8)`.
  Side fix: skip `alloca`/`store` for void-typed locals (was producing
  invalid `alloca void` / `store void`).
- 13 new tests.

### Stage 3.28 — Byte string literal codegen
- **Bug fix**: `b"..."` literals lowered as `Slice(u8)` with `ConstVal::Str`,
  but `Slice` wasn't handled by `mir_type_to_emit_type` (fell through to `I32`),
  and `u8` itself also fell through to `I32`. Wrong types everywhere.
- **Fix**: `TyKind::Slice(elem)` → `Ptr(mir_type_to_emit_type(elem))`.
  `TyKind::Int(I8)` / `Uint(U8)` → `EmitType::I8`.
  `TyKind::Int(I16)` / `Uint(U16)` → `I32` (Stage 3 simplification).
  Byte strings share the same global format as string literals and dedup
  across both.
- 9 new tests.

### Stage 3.23 + 3.26 + 3.29 — Gate Reviews Round 1 + 2 + 3
- R1: 38-case audit (`examples/stage3_gate_audit.rs`), 5/5 APPROVED
- R2: 43-case audit (`examples/stage3_gate_audit_r2.rs`), 5/5 APPROVED
- R3: 43-case audit (`examples/stage3_gate_audit_r3.rs`), 5/5 APPROVED
- §9.3.3 CONVERGED: 3 consecutive rounds with 0 new issues
- L4 (string literals) + L6 (overflow) + L7 (div-by-zero) + L12 (u8/i8 type) CLOSED.
  Remaining: L1 PHI, L2 ADT, L3 closures, L5 traits, L8 lli verification,
  L9 i128, L10 float-bitwise, L11 shift-count, L13 fat pointers, L14 i16, L15 str-as-arg.

### Changed
- `Cargo.toml`: v0.8.5 → v0.8.6
- `src/codegen/emitter.rs`: EmitType refactor + `emit_checked_binop` + `emit_string_global`
- `src/codegen/text_emitter.rs`: updated impls + `emit_checked_binop` + `emit_string_global`
  + `output_with_globals()` + block-scoped cache
- `src/codegen/mod.rs`: real overflow + div-by-zero check emission + string literal codegen
  + interner threaded through all codegen functions + skip void-typed allocas
- `src/mir/body.rs`: `AssertMessage::Overflow(BinOp, Operand, Operand)` + `DivisionByZero(Operand)`
- `src/mir/lower/mod.rs`: `emit_overflow_assert` passes lhs/rhs; new `emit_div_by_zero_assert`
- `tests/codegen_tests.rs`: +52 tests (total 88)
- `tests/deep_inspection.rs`, `tests/integration_stage2_4c.rs`, `examples/round5_deep.rs`: updated pattern matches
- `examples/stage3_gate_audit.rs`, `_r2.rs`, `_r3.rs`: new audit tools
- `docs/develop/v0/stage-3/{dev-log.md, gate-review-round1.md, gate-review-round2.md, gate-review-round3.md}`: full reports

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
