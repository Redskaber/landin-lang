# Stage 15.84 — Test Plan: Borrowck Debug Format Leak Fix

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.208.0 → v0.209.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.84 fixes 3 `{:?}` Debug format leaks in borrowck error
messages:
1. Lifetime error (RegionEscapesUniversal) — `region {:?}` → `region 'rN`
2. Lifetime error (TypeTestFailed) — `type {:?}` → `type T`
3. NotCopy error — `use of moved value: {:?}` → `use of moved value: T`

Also adds a new `region_vid_to_string` helper in `src/mir/ty.rs`.

## 2. New Unit Test (1 test)

Added to `src/mir/ty.rs::tests`:

### 2.1 `region_vid_to_string_basic`

Tests that `RegionVid(N)` formats as `'rN`:

```rust
#[test]
fn region_vid_to_string_basic() {
    assert_eq!(region_vid_to_string(RegionVid(0)), "'r0");
    assert_eq!(region_vid_to_string(RegionVid(1)), "'r1");
    assert_eq!(region_vid_to_string(RegionVid(42)), "'r42");
}
```

## 3. Conformance Test Impact

### 3.1 No conformance test changes

All 5216 conformance tests pass unchanged. The Debug format fixes
don't affect `ERROR_PATTERN` matching because:
- No conformance test checks for `RegionVid(N)` Debug format in
  lifetime errors (lifetime errors are rare and not directly tested
  via ERROR_PATTERN).
- No conformance test checks for `Adt(DefId(N), [])` Debug format in
  NotCopy errors (the ERROR_PATTERN for these tests is "moved" or
  "Copy", not the type display).

## 4. Pipeline Path Coverage (§17.5.1)

### 4.1 Lifetime error path (with human-readable regions)

| Stage | Path | Test |
|-------|------|------|
| Lexer | (no change) | ✅ |
| Parser | (no change) | ✅ |
| HIR lower | (no change) | ✅ |
| MIR lower | (no change) | ✅ |
| Typeck | (no change) | ✅ |
| Borrowck | `region_inference` produces `RegionInferenceError`; `mod.rs` formats with `region_vid_to_string` | ✅ `region_vid_to_string_basic` (unit test for helper) |
| Driver | `to_diagnostics` renders the message | ✅ Existing conformance tests pass |

### 4.2 NotCopy error path (with human-readable type names)

| Stage | Path | Test |
|-------|------|------|
| Lexer | (no change) | ✅ |
| Parser | (no change) | ✅ |
| HIR lower | (no change) | ✅ |
| MIR lower | (no change) | ✅ |
| Typeck | (no change) | ✅ |
| Borrowck | `check_operand` NotCopy arm uses `type_kind_to_string` | ✅ Existing conformance tests pass |
| Driver | `to_diagnostics` renders the message | ✅ Existing conformance tests pass |

## 5. Manual Verification

The lifetime errors are difficult to trigger in isolation (they
require specific region inference scenarios). The NotCopy error
requires a non-Copy type with resolver (not available in test
contexts). Both are verified indirectly:

- **`region_vid_to_string` unit test**: verifies `'r0`, `'r1`, `'r42`
  format correctly.
- **Existing conformance tests**: all 5216 pass, confirming the
  message changes don't break `ERROR_PATTERN` matching.
- **Code review**: the `format!` calls now use `type_kind_to_string`
  and `region_vid_to_string` instead of `{:?}`, matching the pattern
  established in Stage 15.80.

## 6. Regression Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Conformance `ERROR_PATTERN` matches break | LOW | No patterns check for Debug format in these messages |
| `region_vid_to_string` produces wrong format | LOW | 1 unit test verifies exact format for 3 values |
| Lifetime errors become harder to debug | LOW | `'r5` is more readable than `RegionVid(5)` |
| NotCopy errors become harder to debug | LOW | `<adt>` is more readable than `Adt(DefId(3), [])` |

**Overall risk**: LOW. The changes are localized to error message
formatting. The 1 new unit test verifies the new helper.

## 7. Acceptance Criteria (§9.1)

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 233/233 PASS | ✅ 233/233 PASS (was 232, +1 new) |
| `cargo test --features llvm-backend --test all_tests` | 2140/2140 PASS | ✅ 2140/2140 PASS |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |
| Conformance test count | unchanged (5216) | ✅ 5216 |

## 8. Test Sign-off

- ✅ All 233 lib tests pass (was 232, +1 new `region_vid_to_string` test)
- ✅ All 2140 integration tests pass
- ✅ All 5216 conformance tests pass
- ✅ 0 conformance test flips
- ✅ 0 clippy warnings
- ✅ fmt clean

**Stage 15.84 PASSED**.
