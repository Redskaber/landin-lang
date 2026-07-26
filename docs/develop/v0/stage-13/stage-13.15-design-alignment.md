# Stage 13.15 — §13.4 Design Alignment: Fix `landin_main` Double-Prefix Symbol Bug

> **Author**: redskaber
> **Date**: 2026-07-27
> **Process**: stage-committee-process.md v3.21 (§13.4 + §14.4 + §25.8)
> **Baseline**: v0.24.2 / 2317 rust tests + 5026 conformance (Stage 13.14 ✅ eprintln! stderr)
> **Version policy**: v0.24.2 → v0.24.3 (patch bump — linker symbol bug fix)
> **Status**: 🔄 Active — fixes a P0 linker bug discovered during Stage 13.14 smoke testing

---

## 1. Background & Problem Statement

### 1.1 Bug Discovery

While writing a Stage 13.14 behavioral smoke test for `eprintln!`, the following
program failed to link:

```rust
fn landin_main() -> i32 {
    println!("hello world");
    0
}
```

**Linker error**:
```
/usr/bin/ld: landin_wrapper_1258.c:(.text+0xd5): undefined reference to `landin_main'
collect2: error: ld returned 1 exit status
```

**Investigation**: The generated LLVM IR contains:

```llvm
define i32 @landin_landin_main() {
  ...
}
```

The function is named `landin_landin_main` (double `landin_` prefix), but the
C wrapper declares `extern int landin_main(void);` (single prefix). The linker
cannot resolve the symbol.

### 1.2 Root Cause

`src/driver.rs:444` (and parallel sites at `:468`, `:483`):

```rust
let name = interner.try_resolve(&f.ident.name).unwrap_or("fn");
fn_name_by_def_id.insert(*def_id, format!("landin_{}", name));
```

For `fn landin_main()`, `name` resolves to `"landin_main"`, so the symbol becomes
`"landin_" + "landin_main"` = `"landin_landin_main"`.

For `fn main()` (used in all conformance tests), `name` resolves to `"main"`,
so the symbol becomes `"landin_" + "main"` = `"landin_main"` (correct).

### 1.3 Why Stage 13.8/13.9 Tests Didn't Catch This

Stage 13.8 and 13.9 tests verify **source-code presence** of strings like
`extern int landin_main` in `src/bin/main.rs`, but they **do not actually
execute** `--run` on a `fn landin_main()` program. All conformance tests use
`fn main()` (which works correctly), so the bug went unnoticed.

The README's "Hello World" example uses `fn landin_main()`, which is the
**documented entry point** for Landin programs. This means the bug affects
**every user who follows the README** — a P0 user-facing bug.

### 1.4 §15 Long-Term vs Short-Term Analysis

| Option | Long-term value | Short-term cost | Decision |
|--------|----------------|----------------|----------|
| A: Status quo (require `fn main()` in user code) | LOW — contradicts README + breaks user expectation | ZERO | ❌ REJECTED (per §15: long-term > short-term; README contract is sacred) |
| **B: Strip `landin_` prefix if name already starts with it** | **HIGH** — fixes README example; preserves backward compat for `fn main()` users; minimal code change | **LOW** — 1-line change at each of 3 sites in driver.rs | ✅ **ADOPTED** |
| C: Rename entry point to `main()` (Rust convention) | MEDIUM — aligns with Rust convention | HIGH — breaks all existing conformance tests that use `fn main()` (would need to be rewritten as `fn landin_main()` or vice versa); breaks Stage 13.8/13.9 tests | ❌ REJECTED (too disruptive) |
| D: Use a different prefix (e.g., `__landin_`) | LOW — cosmetic; doesn't fix the double-prefix issue | HIGH — breaks all existing symbol references in C wrapper, codegen, vtable | ❌ REJECTED (cascading changes) |

**Conclusion**: Strategy B (strip `landin_` prefix if already present) is the
right call:
- Minimal code change (3 lines)
- Backward compatible (`fn main()` users unaffected)
- Forward compatible (README's `fn landin_main()` works)
- No design-doc change (the `landin_` prefix convention is preserved; we just
  avoid doubling it)

---

## 2. §13.4 Design Alignment Verification

Per `stage-committee-process.md` v3.21 §13.4 "阶段开始时的设计对齐", the following design docs were consulted:

### 2.1 Design Doc Survey

| Design doc | Relevant section | Alignment verdict |
|------------|------------------|-------------------|
| `07-codegen.md` §8.1 | "Codegen translates MIR functions to LLVM `define` with `landin_` prefix" | ✅ ALIGNED — Stage 13.15 preserves the `landin_` prefix convention; just avoids doubling it |
| `08-bootstrap-strategy.md` | "Entry point: `landin_main()` (Landin convention) or `main()` (Rust convention)" | ✅ ALIGNED — Stage 13.15 supports BOTH conventions |
| `13-stage1-feature-whitelist.md` | "Stage 1 source uses `fn landin_main()` as entry point" | ✅ ALIGNED — Stage 13.15 makes `fn landin_main()` work (was broken) |
| `09-stdlib.md` | Mentions `landin_main` as the entry point symbol | ✅ ALIGNED |
| `02-grammar.md` | Silent on entry-point naming | ✅ ALIGNED (no constraint) |

### 2.2 Design-Deviation Classification

Per `stage-committee-process.md` §25.8 design-deviation taxonomy:

- **B1 (impl missing design field)**: NONE
- **B2 (impl has non-design field)**: NONE
- **B3 (impl accepts design-forbidden input)**: NONE
- **B4 (impl introduces design-gray-area)**: NONE — Stage 13.15 is a pure bug fix; no new types, no new fields, no new MIR surface

**Net deviation**: ZERO. Stage 13.15 is a pure implementation bug fix.

### 2.3 §14.4 Six Refactoring Criteria (J1-J6)

| Criterion | Verdict | Rationale |
|-----------|---------|-----------|
| J1 Architectural alignment | ✅ PASS | Restores README contract (`fn landin_main()` works); preserves `landin_` prefix convention |
| J2 Single responsibility | ✅ PASS | The fix is in driver.rs fn_name generation — one job: produce correct LLVM symbol |
| J3 Unidirectional data flow | ✅ PASS | No new data flow; just a string transformation fix |
| J4 Compile-time expressiveness | ✅ PASS | No new types |
| J5 Stage partition (≤5 src files) | ✅ PASS | 1 src file: driver.rs (3 one-line changes at parallel sites) |
| J6 Scientific granularity | ✅ PASS | One bug fix, three identical one-line changes — minimum viable change |

**§14.4 verdict**: 6/6 PASS. No file-count exception needed.

---

## 3. Implementation Blueprint (Strategy B)

### 3.1 Source Files Touched (1 src + 1 test + 1 wiring = 3 files)

| File | Change type | Lines (est.) |
|------|------------|--------------|
| `src/driver.rs` | Strip `landin_` prefix if name already starts with it (3 sites: lines 444, 468, 483) | +9 / -3 |
| `tests/v0/stage13/plan/stage13_15_tests.rs` | NEW — 6 verification tests including a behavioral `--run` test that actually executes a `fn landin_main()` program | +200 |
| `tests/all_tests.rs` | Wire `stage13_15_tests` module | +2 |

### 3.2 The Fix

The current code at 3 sites in `src/driver.rs`:

```rust
let name = interner.try_resolve(&f.ident.name).unwrap_or("fn");
fn_name_by_def_id.insert(*def_id, format!("landin_{}", name));
```

The fix: strip a leading `landin_` from `name` before prefixing (so `landin_main` → `main` → `landin_main`, and `main` → `main` → `landin_main`):

```rust
let raw_name = interner.try_resolve(&f.ident.name).unwrap_or("fn");
// Stage 13.15: Strip a leading "landin_" prefix to avoid doubling it
// (e.g., `fn landin_main()` should produce symbol `landin_main`, not
// `landin_landin_main`). This supports both `fn main()` (Rust convention)
// and `fn landin_main()` (Landin convention) as entry points.
let name = raw_name.strip_prefix("landin_").unwrap_or(raw_name);
fn_name_by_def_id.insert(*def_id, format!("landin_{}", name));
```

### 3.3 Why Not Change the C Wrapper Instead?

An alternative would be to change the C wrapper to call `landin_landin_main`
when the user writes `fn landin_main()`. But this is wrong because:

1. The C wrapper is **static** (compiled into the wrapper source string at
   compile time of the Landin compiler); it doesn't know what the user's
   entry point is named.
2. The C wrapper's `extern int landin_main(void);` is the **public ABI** —
   it's what the linker resolves. Changing this would break all existing
   object files compiled with previous Landin versions.
3. The fix belongs in the **driver** (which knows the user's fn name), not
   the **C wrapper** (which is generic).

### 3.4 API Surface

**No new public API.** Stage 13.15 is a pure bug fix to internal string
formatting in `src/driver.rs`. The `fn_name_by_def_id` map continues to map
`DefId` → `String` (the LLVM symbol name); only the value generation logic
changes.

### 3.5 §16 Interface Isolation Check

- `src/driver.rs`: Modifies 3 string-formatting expressions; no new module-level dependency
- No new module boundaries crossed
- No new pub API

**Verdict**: §16 compliant. No new module boundaries crossed.

---

## 4. Verification Plan

### 4.1 Existing Test Suite (must not regress)

| Suite | Baseline | Expected after Stage 13.15 |
|-------|----------|-----------------------------|
| `cargo test --test all_tests` | 2317 passed | 2317 + 6 (Stage 13.15 tests) = 2323 passed |
| `python3 tests/conformance/run_all.py` | 5026 passed | 5026 passed (no conformance change — all use `fn main()` which already worked) |
| `cargo fmt --check` | clean | clean |
| `cargo clippy --all-targets` | 0 warnings | 0 warnings |

### 4.2 New Stage 13.15 Verification Tests (6 tests)

1. `test_driver_no_double_landin_prefix` — `src/driver.rs` doesn't generate `landin_landin_` symbols (grep for the bug pattern)
2. `test_driver_strips_landin_prefix` — `src/driver.rs` has the `strip_prefix("landin_")` fix at all 3 sites
3. `test_fn_main_still_works` — `fn main()` still produces symbol `landin_main` (no regression for conformance tests)
4. `test_fn_landin_main_now_works` — `fn landin_main()` now produces symbol `landin_main` (not `landin_landin_main`)
5. `test_stage_13_15_design_alignment_exists` — design alignment doc exists
6. `test_stage_13_15_gate_review_exists` — gate review doc exists with PASS verdict
7. `test_v01_gate_still_holds_after_stage_13_15` — ≥5000 conformance .lin files

### 4.3 Behavioral Smoke Test (manual, post-build)

After implementation:

```bash
# Test 1: fn landin_main() (README convention) — was broken, now works
echo 'fn landin_main() -> i32 { println!("hello from landin_main"); 0 }' > /tmp/test1.lin
cargo run --features llvm-backend -- --run /tmp/test1.lin
# Expected stdout: hello from landin_main
# Expected exit: 0

# Test 2: fn main() (Rust convention) — was working, still works (no regression)
echo 'fn main() -> i32 { println!("hello from main"); 0 }' > /tmp/test2.lin
cargo run --features llvm-backend -- --run /tmp/test2.lin
# Expected stdout: hello from main
# Expected exit: 0

# Test 3: eprintln! + landin_main (Stage 13.14 + 13.15 combined)
echo 'fn landin_main() -> i32 { eprintln!("to stderr"); println!("to stdout"); 0 }' > /tmp/test3.lin
cargo run --features llvm-backend -- --run /tmp/test3.lin > /tmp/out.txt 2> /tmp/err.txt
# Expected /tmp/out.txt: to stdout
# Expected /tmp/err.txt: to stderr
```

---

## 5. §25.8 Design Write-Back Plan

Per `stage-committee-process.md` v3.21 §25.8, Stage 13.15 introduces **zero**
new design deviations (it's a pure bug fix to internal string formatting).
Therefore:

| Design doc | Write-back content | Priority |
|------------|-------------------|----------|
| `docs/lang-design/07-codegen.md` | NONE (the `landin_` prefix convention is preserved; just a bug fix to avoid doubling) | — |

**Net write-back**: ZERO. Stage 13.15 is a pure implementation bug fix.

---

## 6. Version Policy

Per `stage-13.1-design-alignment.md` §5.4 version policy framework:

| Stage | Version bump | Rationale |
|-------|-------------|-----------|
| Stage 13.15 | v0.24.2 → v0.24.3 | **Patch bump** — bug fix (linker symbol); no new user-facing feature; no API removal; no design-doc change |

Patch bump justification:
- Bug fix (linker symbol doubling) — not a new feature
- No new language feature
- No new CLI flag
- No new conformance test (5026 unchanged)
- No design-doc write-back (zero new deviations)

---

## 7. Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Conformance tests break (some use `fn main()` which already worked) | LOW (the fix is a no-op when name doesn't start with `landin_`) | LOW | Run conformance suite post-build |
| Vtable method references break (they use `landin_<Type>_<method>` format) | LOW (the fix only strips prefix when name == `landin_*`; vtable names like `landin_SomeType_method` don't start with `landin_` after the type prefix... wait, they DO start with `landin_`) | MEDIUM | Inspect vtable codegen paths carefully |
| Stage 13.8/13.9 tests break (they check for `extern int landin_main` in C wrapper) | ZERO (C wrapper is unchanged) | — | No action needed |
| Other code paths generate fn names with `landin_` prefix | MEDIUM (need to audit all `format!("landin_{}", ...)` sites) | LOW | Grep audit + test |

**Overall risk**: LOW-MEDIUM. The vtable concern (row 2) needs investigation.

### 7.1 Vtable Audit

The vtable codegen at `src/driver.rs:483`:

```rust
return Some(format!("landin_{}_{}", type_str, method));
```

This generates names like `landin_MyType_my_method`. The `type_str` here is the
type name (e.g., `MyType`), not `landin_MyType`. So this site is **not affected**
by the double-prefix bug (the type name doesn't start with `landin_`).

However, the fix at this site should still be applied for consistency (in case
a user ever names their type `landin_Foo`, which would produce `landin_landin_Foo_method`).

---

## 8. Stage Committee Recommendation

**GO** — proceed with implementation.

Conditions:
1. ✅ §13.4 design alignment complete (this document)
2. ✅ §14.4 J1-J6 all PASS (6/6)
3. ✅ §16 interface isolation preserved
4. ✅ §25.8 write-back plan documented (zero new deviations)
5. ✅ Version policy: v0.24.2 → v0.24.3 (patch bump, justified)
6. ✅ Test plan: 6 new verification tests + 2317 existing tests + 5026 conformance
7. ⚠️ Vtable audit: confirm vtable codegen paths don't break (row 2 of risk assessment)

No conditions blocking implementation. Proceed to gate-review-13.15.md → implementation → CI/CD.

---

## 9. Next Steps

| Step | Action | Owner | Estimated |
|------|--------|-------|-----------|
| 1 | Create `docs/develop/v0/stage-13/gate-review-13.15.md` | REV-A | 20 min |
| 2 | Implement Strategy B (1 src file, 3 sites) | DEV-A | 15 min |
| 3 | Create `tests/v0/stage13/plan/stage13_15_tests.rs` (6 tests) | QA-A | 30 min |
| 4 | Wire `stage13_15_tests` into `tests/all_tests.rs` | DEV-A | 2 min |
| 5 | Bump `Cargo.toml` v0.24.2 → v0.24.3 | DEV-A | 1 min |
| 6 | Run full CI/CD (cargo clean + build + fmt + clippy + test) | QA-A | 30 min |
| 7 | Run behavioral smoke test (3 scenarios from §4.3) | QA-A | 15 min |
| 8 | Update `docs/llvm/execution-pipeline.md`, `README.md`, `RELEASE_NOTES.md`, `api-naming-standard.md`, `docs/tests/matrix.md`, `docs/tests/v0/stage13/plan/README.md`, `docs/worklog.md` | REC-A | 1 hour |
| 9 | Create zip package | DEV-A | 5 min |

**Total estimated**: ~3 hours.

---

## 10. Lessons Applied

From Stage 13.8/13.9 retrospective:
- **Lesson**: Tests that check source-code presence of strings (`content.contains("extern int landin_main")`) don't catch behavioral bugs. Always include at least one test that **actually executes** the feature.
- **Applied**: Stage 13.15 includes a behavioral test (`test_fn_landin_main_now_works`) that compiles + links + runs a `fn landin_main()` program and verifies the output.

From Stage 13.14 retrospective:
- **Lesson**: When adding new features, always smoke-test with the README's documented entry point (`fn landin_main()`), not just the conformance tests' entry point (`fn main()`).
- **Applied**: Stage 13.15 was discovered by following the README's hello-world example verbatim.

---

## 11. References

- `stage-committee-process.md` v3.21 §13.4, §14.4, §15, §16, §25.8
- `docs/develop/v0/stage-13/stage-13.14-design-alignment.md` (Stage 13.14 — predecessor; smoke test revealed this bug)
- `src/driver.rs:444,468,483` (the 3 sites with `format!("landin_{}", name)` — modification target)
- `src/bin/main.rs:170` (C wrapper `extern int landin_main(void);` — the symbol the linker expects)
- `tests/v0/stage13/plan/stage13_8_tests.rs` (Stage 13.8 tests — source-presence-only, didn't catch this bug)
- `tests/v0/stage13/plan/stage13_9_tests.rs` (Stage 13.9 tests — source-presence-only, didn't catch this bug)
- `README.md` (uses `fn landin_main()` as entry point — was broken before Stage 13.15)
