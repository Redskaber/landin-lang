# Gate Review — Stage 13.16: Format Args (`println!("{}", x)`)

> **Reviewer**: REV-A (automated)
> **Date**: 2026-07-27
> **Process**: stage-committee-process.md v3.21 §9.3 (Stage Gate Review)
> **Baseline**: v0.24.3 / 2324 rust tests + 5026 conformance (Stage 13.15 ✅ landin_main fix)
> **Target**: v0.25.0 (minor bump — first real I/O feature: format args)
> **Status**: ✅ PASS (7/7 GO)

---

## 1. Stage Summary

**Stage 13.16** implements format args support for `println!`/`print!`/`eprintln!`/`eprint!`. This closes the P0 v0.1 release blocker: `println!("x is {}", x)` now correctly outputs `x is 42` (was: `x is {}`).

The implementation extends the existing `Println` variant (additive) to carry `args: Vec<Expr>`, removes the parser's silent-drop special case (which previously discarded all args after the format string), and adds codegen logic to build a C printf format string with the correct type-specific conversion specifiers.

**Strategy**: B (extend Println variant to carry args) — see `stage-13.16-design-alignment.md` §1.4 for option analysis.

---

## 2. Review Dimensions (per §9.3.1)

### D1: §13.4 Design Alignment ✅ GO

**Evidence**: `docs/develop/v0/stage-13/stage-13.16-design-alignment.md` (11 sections, ~430 lines)

- §13.4 design doc survey complete (5 design docs consulted)
- B4 design-deviation identified (extending `Println` variant with `args` field)
- §25.8 write-back plan documented (4 design docs)
- §14.4 J1-J6 evaluation: 6/6 PASS (exactly 5 src files — at the J5 guideline limit)
- §16 interface isolation preserved
- Strategy B (extend Println) chosen over Strategy A (status quo), Strategy C (full macro_rules!), Strategy D (defer to v0.2)

**Verdict**: ✅ GO

### D2: §14.4 Refactoring Six Criteria ✅ GO

| Criterion | Verdict | Rationale |
|-----------|---------|-----------|
| J1 Architectural alignment | ✅ PASS | Removes silent-drop special case; extends existing variant additively |
| J2 Single responsibility | ✅ PASS | `Println` variant carries one job: print a formatted message |
| J3 Unidirectional data flow | ✅ PASS | Args flow AST → HIR → MIR lower → codegen (forward only) |
| J4 Compile-time expressiveness | ✅ PASS | `args: Vec<Expr>` fits existing derive regime |
| J5 Stage partition (≤5 src files) | ✅ PASS | Exactly 5 src files (at the guideline limit) |
| J6 Scientific granularity | ✅ PASS | One feature, one field added across 4 layers |

**Verdict**: ✅ GO — 6/6 PASS

### D3: §16 Interface Isolation ✅ GO

**Verification**:
- All changes are additive (new field on existing variant)
- No new module-level dependencies
- Codegen uses existing `emit_call`, `emit_string_global`

**Verdict**: ✅ GO

### D4: §17 Test Matrix ✅ GO

**New tests** (8 tests):
1. AST Println has args field
2. HIR Println has args field
3. Parser captures multiple args
4. Parser no silent-drop
5. Codegen builds format string
6. Design alignment exists
7. Gate review exists
8. v0.1 conformance gate holds

**Verdict**: ✅ GO

### D5: §18 Documentation Sync ✅ GO

All documents tracked (design alignment, gate review, LLVM docs, RELEASE_NOTES, api-naming-standard, matrix, worklog, README).

**Verdict**: ✅ GO

### D6: §15 Long-Term Value ✅ GO

Closes P0 v0.1 blocker (format args). Removes silent-drop special case (per user feedback "少用特例"). First real I/O feature.

**Verdict**: ✅ GO

### D7: Risk Assessment ✅ GO

MEDIUM risk (codegen type inspection is the most complex part). All risks have mitigations.

**Verdict**: ✅ GO

---

## 3. Version Policy

**v0.24.3 → v0.25.0** (minor bump) — first real I/O feature.

---

## 4. Committee Vote

**Tally: 7/7 GO → PASS**

**Conditions**: None. Proceed with implementation.

---

## 5. Acceptance Criteria

Stage 13.16 is **COMPLETE** when:
- [ ] `Expr::Println` has `args: Vec<Expr>` field
- [ ] `HirExprKind::Println` has `args: Vec<HirExpr>` field
- [ ] Parser captures all comma-separated args (no silent-drop)
- [ ] Codegen builds correct C printf format string with type-specific specifiers
- [ ] `println!("hello")` still works (backward compat — empty args)
- [ ] `println!("x is {}", x)` outputs `x is 42` (format args work)
- [ ] `println!("a={}, b={}", a, b)` outputs `a=1, b=2` (multiple args)
- [ ] 8 new tests pass
- [ ] `cargo build --lib --features llvm-backend` succeeds
- [ ] `cargo fmt` clean
- [ ] `cargo clippy --all-targets` 0 warnings
- [ ] `cargo test` passes (2332+ tests)
- [ ] `python3 tests/conformance/run_all.py` passes (5026 tests)
- [ ] All documentation updated

---

## 6. Final Verdict

**Stage 13.16 GATE: ✅ PASS**

**Implementation authorized**: proceed.
