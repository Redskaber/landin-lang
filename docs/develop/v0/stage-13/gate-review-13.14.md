# Gate Review — Stage 13.14: eprintln!/eprint! Stderr Emission

> **Reviewer**: REV-A (automated)
> **Date**: 2026-07-27
> **Process**: stage-committee-process.md v3.21 §9.3 (Stage Gate Review)
> **Baseline**: v0.24.1 / 2310 rust tests + 5026 conformance (Stage 13.13 ✅ inline println!)
> **Target**: v0.24.2 (patch bump — completes Stage 13.13 deferral)
> **Status**: ✅ PASS (7/7 GO)

---

## 1. Stage Summary

**Stage 13.14** closes the explicit deferral from Stage 13.13: the `stderr` flag on `StatementKind::Println` is now exercised at codegen time. When `stderr == true` (i.e., `eprintln!` or `eprint!` was invoked), codegen emits a call to a new `__landin_eprint` C wrapper helper that routes the message to stderr via `fprintf(stderr, "%s", s)`. When `stderr == false` (i.e., `println!` or `print!`), the existing `printf` path is unchanged.

This restores Rust semantics for `eprintln!`/`eprint!`: error/diagnostic messages go to stderr (unbuffered, separate from stdout data), enabling proper pipe redirection and POSIX convention compliance.

**Strategy**: B (`__landin_eprint` helper) — see `stage-13.14-design-alignment.md` §1.3 for option analysis.

---

## 2. Review Dimensions (per §9.3.1)

### D1: §13.4 Design Alignment ✅ GO

**Evidence**: `docs/develop/v0/stage-13/stage-13.14-design-alignment.md` (11 sections, ~360 lines)

- §13.4 design doc survey complete (5 design docs consulted)
- Zero new design deviations (Stage 13.14 exercises the existing `stderr` field from Stage 13.13)
- §25.8 write-back: ZERO new deviations → zero write-back required
- §14.4 J1-J6 evaluation: 6/6 PASS (no file-count exception needed)
- §16 interface isolation preserved (no new module boundaries crossed)
- Strategy B (`__landin_eprint` helper) chosen over Strategy A (direct `fprintf` + `stderr` extern — portability risk), Strategy C (defer to v0.2 macro_rules! — design-forbidden), Strategy D (status quo — known correctness bug)

**Verdict**: ✅ GO — design alignment complete and rigorous

### D2: §14.4 Refactoring Six Criteria ✅ GO

| Criterion | Verdict | Rationale |
|-----------|---------|-----------|
| J1 Architectural alignment | ✅ PASS | Restores Rust semantics for `eprintln!`/`eprint!` (stderr, not stdout) |
| J2 Single responsibility | ✅ PASS | `__landin_eprint` helper carries one job: print to stderr |
| J3 Unidirectional data flow | ✅ PASS | MIR lower → MIR body → codegen → C wrapper helper → libc; all forward |
| J4 Compile-time expressiveness | ✅ PASS | No new types; just a branch on existing `bool` field |
| J5 Stage partition (≤5 src files) | ✅ PASS | 2 src files: codegen/mod.rs (1 branch), bin/main.rs (1 helper) |
| J6 Scientific granularity | ✅ PASS | One bug fix, one branch, one helper — minimum viable change |

**Verdict**: ✅ GO — 6/6 PASS, no exceptions required

### D3: §16 Interface Isolation ✅ GO

**Verification**:
- `src/codegen/mod.rs`: Adds a branch on existing `bool` field; calls existing `emitter.emit_call` (already public); no new codegen → MIR back-edge
- `src/bin/main.rs`: Adds a new C helper function to the wrapper source string; no new Rust-side dependency
- No new module boundaries crossed
- No new pub API on the Rust side (the helper lives in C, called via `emit_call`)

**Verdict**: ✅ GO — §16 compliant

### D4: §17 Test Matrix ✅ GO

**Existing tests (must not regress)**:
- `cargo test --test all_tests`: 2310 passed (baseline) → expected 2316 passed (2310 + 6 new)
- `python3 tests/conformance/run_all.py`: 5026 passed (no conformance change)
- `cargo fmt --check`: clean
- `cargo clippy --all-targets`: 0 warnings

**New tests** (`tests/v0/stage13/plan/stage13_14_tests.rs`, 7 tests):
1. `test_codegen_println_branches_on_stderr` — codegen Println arm has `if stderr` branch
2. `test_codegen_eprint_calls_helper` — `stderr == true` calls `__landin_eprint`
3. `test_codegen_stdout_unchanged` — `stderr == false` still calls `printf` (no regression)
4. `test_c_wrapper_has_eprint_helper` — C wrapper defines `__landin_eprint` with `fprintf(stderr, ...)`
5. `test_stage_13_14_design_alignment_exists` — design doc exists with required sections
6. `test_stage_13_14_gate_review_exists` — this document exists with PASS verdict
7. `test_v01_gate_still_holds_after_stage_13_14` — ≥5000 conformance

**Verdict**: ✅ GO — test matrix complete; 7 new tests + 2310 existing + 5026 conformance

### D5: §18 Documentation Sync ✅ GO

**Documents created/updated**:

| Document | Action | Status |
|----------|--------|--------|
| `docs/develop/v0/stage-13/stage-13.14-design-alignment.md` | NEW | ✅ Created (~360 lines) |
| `docs/develop/v0/stage-13/gate-review-13.14.md` | NEW | ✅ Created (this file) |
| `docs/llvm/stage-13.14-eprintln-stderr-emission.md` | NEW | ⏳ Pending implementation |
| `docs/llvm/README.md` | UPDATE | ⏳ Pending (add link to new doc) |
| `docs/llvm/execution-pipeline.md` | UPDATE | ⏳ Pending (note stderr routing) |
| `docs/develop/v0/api-naming-standard.md` | UPDATE | ⏳ Pending (v2.46 → v2.47) |
| `docs/tests/matrix.md` | UPDATE | ⏳ Pending (Stage 13.14 row) |
| `docs/tests/v0/stage13/plan/README.md` | UPDATE | ⏳ Pending (Stage 13.14 row) |
| `docs/worklog.md` | APPEND | ⏳ Pending (Stage 13.14 entry) |
| `RELEASE_NOTES.md` | APPEND | ⏳ Pending (v0.24.2 entry) |
| `README.md` | REWRITE | ⏳ Pending (full refresh) |
| `Cargo.toml` | BUMP | ⏳ Pending (v0.24.1 → v0.24.2) |
| `tests/all_tests.rs` | APPEND | ⏳ Pending (wire stage13_14_tests module) |

**§25.8 design write-back**: ZERO new deviations → ZERO write-back required.

**Verdict**: ✅ GO — documentation plan complete; all updates tracked

### D6: §15 Long-Term Value ✅ GO

**§15 "最优 > 最小" (long-term > short-term) analysis**:

Stage 13.13 chose to defer the `stderr` flag to Stage 13.14 — a deliberate incremental decision per §15.3 #3 (前置条件未就绪). Stage 13.14 closes that deferral with the architecturally-correct approach:

- **Strategy A** (direct `fprintf` + `stderr` extern): rejected due to portability risk (`stderr` is a macro in glibc, not a simple global)
- **Strategy B** (`__landin_eprint` helper): adopted — portable, symmetric with existing `__landin_panic_*` helpers, minimal codegen change

This restores Rust semantics for `eprintln!`/`eprint!`, which matters for:
- Pipe redirection (`./prog > out.txt` should not capture stderr)
- Convention compliance (POSIX tools expect diagnostics on stderr)
- Buffering semantics (stderr is unbuffered; stdout is line-buffered)

**§15 verdict**: Long-term value clearly outweighs short-term cost. ✅ GO.

### D7: Risk Assessment ✅ GO

Per `stage-13.14-design-alignment.md` §7:
- All risks LOW or ZERO
- No blocking risks identified
- Existing test coverage (2310 rust + 5026 conformance) provides strong regression protection

**Verdict**: ✅ GO — risk profile acceptable

---

## 3. Version Policy

**v0.24.1 → v0.24.2** (patch bump)

Justification:
- Bug fix (stderr routing) — not a new feature
- No new language feature (eprintln! was already "working" in 13.13, just to wrong stream)
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
| J5 Stage partition (≤5 src files) | ✅ PASS (2 files) |
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
| 1 | Modify `StatementKind::Println` arm in `src/codegen/mod.rs` to branch on `stderr` flag | 15 min |
| 2 | Add `__landin_eprint` helper to C wrapper in `src/bin/main.rs` | 5 min |
| 3 | Create `tests/v0/stage13/plan/stage13_14_tests.rs` (7 tests) | 30 min |
| 4 | Wire `stage13_14_tests` into `tests/all_tests.rs` | 2 min |
| 5 | Bump `Cargo.toml` v0.24.1 → v0.24.2 | 1 min |
| 6 | Run `cargo clean && cargo build --lib --features llvm-backend && cargo fmt && cargo clippy --all-targets && cargo test` | 30 min |
| 7 | Create `docs/llvm/stage-13.14-eprintln-stderr-emission.md` + update README + execution-pipeline | 30 min |
| 8 | Update `api-naming-standard.md`, `docs/tests/matrix.md`, `docs/tests/v0/stage13/plan/README.md`, `docs/worklog.md`, `RELEASE_NOTES.md` | 30 min |
| 9 | Rewrite `README.md` | 20 min |
| 10 | Create zip package | 5 min |

**Total estimated**: ~2.5 hours.

---

## 7. Acceptance Criteria

Stage 13.14 is **COMPLETE** when all of the following hold:

- [ ] `StatementKind::Println` arm in `src/codegen/mod.rs` has an `if stderr { ... } else { ... }` branch
- [ ] When `stderr == true`, codegen calls `__landin_eprint` via `emitter.emit_call`
- [ ] When `stderr == false`, codegen still calls `printf` (Stage 13.13 path, unchanged)
- [ ] C wrapper in `src/bin/main.rs` defines `__landin_eprint(const char* s)` with `fprintf(stderr, "%s", s)` body
- [ ] `tests/v0/stage13/plan/stage13_14_tests.rs` exists with 7 tests
- [ ] `tests/all_tests.rs` wires `stage13_14_tests` module
- [ ] `Cargo.toml` version is `0.24.2`
- [ ] `cargo build --lib --features llvm-backend` succeeds
- [ ] `cargo fmt` succeeds (no changes)
- [ ] `cargo clippy --all-targets` returns 0 warnings
- [ ] `cargo test` passes (expected 2316+ tests)
- [ ] `python3 tests/conformance/run_all.py` passes (5026 tests)
- [ ] `docs/develop/v0/stage-13/gate-review-13.14.md` exists (this file)
- [ ] `docs/develop/v0/stage-13/stage-13.14-design-alignment.md` exists
- [ ] `docs/llvm/stage-13.14-eprintln-stderr-emission.md` exists
- [ ] `docs/llvm/README.md` references the new doc
- [ ] `RELEASE_NOTES.md` has v0.24.2 entry
- [ ] `docs/worklog.md` has Stage 13.14 entry
- [ ] `api-naming-standard.md` has v2.47 entry
- [ ] `docs/tests/matrix.md` has Stage 13.14 row
- [ ] `docs/tests/v0/stage13/plan/README.md` has Stage 13.14 row
- [ ] `README.md` rewritten to reflect current state
- [ ] Zip package created in `/home/z/my-project/download/`

---

## 8. Post-Stage TODO (deferred to future stages)

- Stage 13.15: Format string support (`println!("{}", x)`) — requires HIR-time format-args expansion
- Stage 13.16: String escape sequences in lexer (`\n`, `\t`, `\\`, `\"`) — affects all string literals
- Stage 13.17: `print!` (no newline) flush behavior — currently `newline: false` is captured but doesn't affect codegen
- v0.2+: Full `macro_rules!` expansion (replaces Stage 13.13/13.14 inline approach)

---

## 9. Lessons Applied

From Stage 13.13 retrospective:
- **Lesson**: When adding a new codegen arm, capture all relevant semantic flags as fields on the MIR variant — even if not yet exercised. This makes future refinements (like Stage 13.14) trivially additive.
- **Applied**: Stage 13.13 captured `stderr: bool` on `StatementKind::Println` even though it was unused; Stage 13.14 just exercises the existing field with zero new MIR surface.

From Stage 13.10 retrospective (C wrapper helpers):
- **Lesson**: When C wrapper helpers are needed, follow the `__landin_<verb>_<noun>` naming pattern and keep them minimal (1-line bodies). This makes the helper API surface auditable.
- **Applied**: `__landin_eprint` follows the pattern; 1-line body (`fprintf(stderr, "%s", s)`).

---

## 10. Final Verdict

**Stage 13.14 GATE: ✅ PASS**

**Committee vote**: 7/7 GO, 0 conditions blocking.

**Implementation authorized**: proceed.

---

## References

- `docs/develop/v0/stage-13/stage-13.14-design-alignment.md` (companion design doc)
- `docs/develop/v0/stage-13/stage-13.13-design-alignment.md` (Stage 13.13 — added the `stderr` field that Stage 13.14 exercises)
- `docs/stage-committee-process.md` v3.21 §9.3 (gate review protocol)
- `docs/stage-committee-process.md` v3.21 §13.4 (design alignment protocol)
- `docs/stage-committee-process.md` v3.21 §14.4 (refactoring criteria)
- `docs/stage-committee-process.md` v3.21 §16 (interface isolation)
- `docs/stage-committee-process.md` v3.21 §25.8 (design write-back)
