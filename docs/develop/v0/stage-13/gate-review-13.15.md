# Gate Review — Stage 13.15: Fix `landin_main` Double-Prefix Symbol Bug

> **Reviewer**: REV-A (automated)
> **Date**: 2026-07-27
> **Process**: stage-committee-process.md v3.21 §9.3 (Stage Gate Review)
> **Baseline**: v0.24.2 / 2317 rust tests + 5026 conformance (Stage 13.14 ✅ eprintln! stderr)
> **Target**: v0.24.3 (patch bump — linker symbol bug fix)
> **Status**: ✅ PASS (7/7 GO)

---

## 1. Stage Summary

**Stage 13.15** fixes a P0 linker bug discovered during Stage 13.14 smoke
testing: the README's "Hello World" example (`fn landin_main() { ... }`)
failed to link with `undefined reference to 'landin_main'`.

**Root cause**: `src/driver.rs` generates LLVM symbols by prefixing function
names with `landin_` (e.g., `format!("landin_{}", name)`). For `fn main()`,
this produces `landin_main` (correct). For `fn landin_main()`, this produces
`landin_landin_main` (double prefix — wrong). The C wrapper expects
`landin_main`, so the linker fails.

**Fix**: Strip a leading `landin_` from the function name before prefixing
(at 3 sites in `src/driver.rs`). This makes both `fn main()` (Rust convention)
and `fn landin_main()` (Landin convention) produce the same LLVM symbol
`landin_main`, matching the C wrapper's `extern int landin_main(void);`
declaration.

**Strategy**: B (strip `landin_` prefix if already present) — see
`stage-13.15-design-alignment.md` §1.4 for option analysis.

---

## 2. Review Dimensions (per §9.3.1)

### D1: §13.4 Design Alignment ✅ GO

**Evidence**: `docs/develop/v0/stage-13/stage-13.15-design-alignment.md` (11 sections, ~370 lines)

- §13.4 design doc survey complete (5 design docs consulted)
- Zero new design deviations (Stage 13.15 is a pure bug fix to internal string formatting)
- §25.8 write-back: ZERO new deviations → zero write-back required
- §14.4 J1-J6 evaluation: 6/6 PASS (no file-count exception needed)
- §16 interface isolation preserved (no new module boundaries crossed)
- Strategy B (strip prefix) chosen over Strategy A (status quo), Strategy C (rename to `main()`), Strategy D (different prefix)

**Verdict**: ✅ GO — design alignment complete and rigorous

### D2: §14.4 Refactoring Six Criteria ✅ GO

| Criterion | Verdict | Rationale |
|-----------|---------|-----------|
| J1 Architectural alignment | ✅ PASS | Restores README contract (`fn landin_main()` works); preserves `landin_` prefix convention |
| J2 Single responsibility | ✅ PASS | The fix is in driver.rs fn_name generation — one job: produce correct LLVM symbol |
| J3 Unidirectional data flow | ✅ PASS | No new data flow; just a string transformation fix |
| J4 Compile-time expressiveness | ✅ PASS | No new types |
| J5 Stage partition (≤5 src files) | ✅ PASS | 1 src file: driver.rs (3 one-line changes at parallel sites) |
| J6 Scientific granularity | ✅ PASS | One bug fix, three identical one-line changes — minimum viable change |

**Verdict**: ✅ GO — 6/6 PASS, no exceptions required

### D3: §16 Interface Isolation ✅ GO

**Verification**:
- `src/driver.rs`: Modifies 3 string-formatting expressions; no new module-level dependency
- No new module boundaries crossed
- No new pub API

**Verdict**: ✅ GO — §16 compliant

### D4: §17 Test Matrix ✅ GO

**Existing tests (must not regress)**:
- `cargo test --test all_tests`: 2317 passed (baseline) → expected 2323 passed (2317 + 6 new)
- `python3 tests/conformance/run_all.py`: 5026 passed (no conformance change — all use `fn main()` which already worked)
- `cargo fmt --check`: clean
- `cargo clippy --all-targets`: 0 warnings

**New tests** (`tests/v0/stage13/plan/stage13_15_tests.rs`, 7 tests):
1. `test_driver_no_double_landin_prefix` — grep for `landin_landin` pattern (must be 0)
2. `test_driver_strips_landin_prefix` — `src/driver.rs` has `strip_prefix("landin_")` fix
3. `test_fn_main_still_works` — `fn main()` produces `landin_main` (no regression)
4. `test_fn_landin_main_now_works` — `fn landin_main()` produces `landin_main` (bug fix)
5. `test_stage_13_15_design_alignment_exists` — design doc exists
6. `test_stage_13_15_gate_review_exists` — this document exists with PASS verdict
7. `test_v01_gate_still_holds_after_stage_13_15` — ≥5000 conformance

**Verdict**: ✅ GO — test matrix complete; 7 new tests + 2317 existing + 5026 conformance

### D5: §18 Documentation Sync ✅ GO

**Documents created/updated**:

| Document | Action | Status |
|----------|--------|--------|
| `docs/develop/v0/stage-13/stage-13.15-design-alignment.md` | NEW | ✅ Created (~370 lines) |
| `docs/develop/v0/stage-13/gate-review-13.15.md` | NEW | ✅ Created (this file) |
| `docs/llvm/execution-pipeline.md` | UPDATE | ⏳ Pending (note the bug fix) |
| `docs/develop/v0/api-naming-standard.md` | UPDATE | ⏳ Pending (v2.47 → v2.48) |
| `docs/tests/matrix.md` | UPDATE | ⏳ Pending (Stage 13.15 row) |
| `docs/tests/v0/stage13/plan/README.md` | UPDATE | ⏳ Pending (Stage 13.15 row) |
| `docs/worklog.md` | APPEND | ⏳ Pending (Stage 13.15 entry) |
| `RELEASE_NOTES.md` | APPEND | ⏳ Pending (v0.24.3 entry) |
| `README.md` | REWRITE | ⏳ Pending (full refresh) |
| `Cargo.toml` | BUMP | ⏳ Pending (v0.24.2 → v0.24.3) |
| `tests/all_tests.rs` | APPEND | ⏳ Pending (wire stage13_15_tests module) |

**§25.8 design write-back**: ZERO new deviations → ZERO write-back required.

**Verdict**: ✅ GO — documentation plan complete; all updates tracked

### D6: §15 Long-Term Value ✅ GO

**§15 "最优 > 最小" (long-term > short-term) analysis**:

The bug affects **every user who follows the README** (which uses `fn landin_main()` as the entry point). Stage 13.15 fixes this with a 1-line-per-site change (3 sites total), preserving backward compatibility for `fn main()` users (all conformance tests).

Without this fix, the README's hello-world example fails to link — a P0 user-facing bug.

**§15 verdict**: Long-term value clearly outweighs short-term cost. ✅ GO.

### D7: Risk Assessment ✅ GO

Per `stage-13.15-design-alignment.md` §7:
- All risks LOW or ZERO (vtable concern investigated and dismissed — vtable type names don't start with `landin_`)
- No blocking risks identified
- Existing test coverage (2317 rust + 5026 conformance) provides strong regression protection

**Verdict**: ✅ GO — risk profile acceptable

---

## 3. Version Policy

**v0.24.2 → v0.24.3** (patch bump)

Justification:
- Bug fix (linker symbol doubling) — not a new feature
- No new language feature
- No new CLI flag
- No new conformance test (5026 unchanged)
- No design-doc write-back (zero new deviations)

Per `stage-13.1-design-alignment.md` §5.4 version policy framework, this matches the "patch bump" category.

---

## 4. §14.4 J1-J6 Final Verdict

| Criterion | Status |
|-----------|--------|
| J1 Architectural alignment | ✅ PASS |
| J2 Single responsibility | ✅ PASS |
| J3 Unidirectional data flow | ✅ PASS |
| J4 Compile-time expressiveness | ✅ PASS |
| J5 Stage partition (≤5 src files) | ✅ PASS (1 file) |
| J6 Scientific granularity | ✅ PASS |

**All 6 criteria PASS. No exceptions required.**

---

## 5. Committee Vote

| Reviewer | Vote | Condition |
|----------|------|-----------|
| D1 §13.4 Design Alignment | ✅ GO | None |
| D2 §14.4 Refactoring Criteria | ✅ GO | None |
| D3 §16 Interface Isolation | ✅ GO | None |
| D4 §17 Test Matrix | ✅ GO | None |
| D5 §18 Documentation Sync | ✅ GO | None |
| D6 §15 Long-term Value | ✅ GO | None |
| D7 Risk Assessment | ✅ GO | None |

**Tally: 7/7 GO → PASS**

**Conditions**: None. Proceed with implementation.

---

## 6. Implementation Plan

| Step | Action | Estimated |
|------|--------|-----------|
| 1 | Modify 3 sites in `src/driver.rs` to strip `landin_` prefix | 10 min |
| 2 | Create `tests/v0/stage13/plan/stage13_15_tests.rs` (7 tests) | 30 min |
| 3 | Wire `stage13_15_tests` into `tests/all_tests.rs` | 2 min |
| 4 | Bump `Cargo.toml` v0.24.2 → v0.24.3 | 1 min |
| 5 | Run `cargo clean && cargo build --lib --features llvm-backend && cargo fmt && cargo clippy --all-targets && cargo test` | 30 min |
| 6 | Run behavioral smoke test (3 scenarios) | 15 min |
| 7 | Update `docs/llvm/execution-pipeline.md`, `api-naming-standard.md`, `docs/tests/matrix.md`, `docs/tests/v0/stage13/plan/README.md`, `docs/worklog.md`, `RELEASE_NOTES.md`, `README.md` | 1 hour |
| 8 | Create zip package | 5 min |

**Total estimated**: ~2.5 hours.

---

## 7. Acceptance Criteria

Stage 13.15 is **COMPLETE** when all of the following hold:

- [ ] `src/driver.rs` has `strip_prefix("landin_")` at all 3 fn_name generation sites (lines ~444, ~468, ~483)
- [ ] `src/driver.rs` does NOT contain `format!("landin_{}", name)` without the strip_prefix (grep returns 0 unfixed sites)
- [ ] `tests/v0/stage13/plan/stage13_15_tests.rs` exists with 7 tests
- [ ] `tests/all_tests.rs` wires `stage13_15_tests` module
- [ ] `Cargo.toml` version is `0.24.3`
- [ ] `cargo build --lib --features llvm-backend` succeeds
- [ ] `cargo fmt` succeeds (no changes)
- [ ] `cargo clippy --all-targets` returns 0 warnings
- [ ] `cargo test` passes (expected 2323+ tests)
- [ ] `python3 tests/conformance/run_all.py` passes (5026 tests)
- [ ] **Behavioral**: `fn landin_main() -> i32 { println!("hello"); 0 }` compiles, links, runs, and prints "hello" to stdout
- [ ] **Behavioral**: `fn main() -> i32 { println!("hello"); 0 }` still works (no regression)
- [ ] `docs/develop/v0/stage-13/gate-review-13.15.md` exists (this file)
- [ ] `docs/develop/v0/stage-13/stage-13.15-design-alignment.md` exists
- [ ] `RELEASE_NOTES.md` has v0.24.3 entry
- [ ] `docs/worklog.md` has Stage 13.15 entry
- [ ] `api-naming-standard.md` has v2.48 entry
- [ ] `docs/tests/matrix.md` has Stage 13.15 row
- [ ] `docs/tests/v0/stage13/plan/README.md` has Stage 13.15 row
- [ ] `README.md` rewritten to reflect current state
- [ ] Zip package created in `/home/z/my-project/download/`

---

## 8. Post-Stage TODO (deferred to future stages)

- Stage 13.16: String escape sequences in lexer (`\n`, `\t`, `\\`, `\"`) — INVESTIGATE: appears already working (verified during Stage 13.15 bug investigation); may not need a stage
- Stage 13.17: Format string support (`println!("{}", x)`) — requires HIR-time format-args expansion
- Stage 13.18: `print!` (no newline) flush behavior
- v0.2+: Full `macro_rules!` expansion (replaces Stage 13.13/13.14 inline approach)

---

## 9. Lessons Applied

From Stage 13.8/13.9 retrospective:
- **Lesson**: Tests that check source-code presence of strings don't catch behavioral bugs. Always include at least one test that **actually executes** the feature.
- **Applied**: Stage 13.15 includes behavioral tests that compile + link + run actual Landin programs.

From Stage 13.14 retrospective:
- **Lesson**: When adding new features, always smoke-test with the README's documented entry point (`fn landin_main()`), not just the conformance tests' entry point (`fn main()`).
- **Applied**: Stage 13.15 was discovered by following the README's hello-world example verbatim.

---

## 10. Final Verdict

**Stage 13.15 GATE: ✅ PASS**

**Committee vote**: 7/7 GO, 0 conditions blocking.

**Implementation authorized**: proceed.

---

## References

- `docs/develop/v0/stage-13/stage-13.15-design-alignment.md` (companion design doc)
- `docs/develop/v0/stage-13/stage-13.14-design-alignment.md` (Stage 13.14 — predecessor; smoke test revealed this bug)
- `docs/stage-committee-process.md` v3.21 §9.3 (gate review protocol)
- `docs/stage-committee-process.md` v3.21 §13.4 (design alignment protocol)
- `docs/stage-committee-process.md` v3.21 §14.4 (refactoring criteria)
- `docs/stage-committee-process.md` v3.21 §16 (interface isolation)
- `docs/stage-committee-process.md` v3.21 §25.8 (design write-back)
