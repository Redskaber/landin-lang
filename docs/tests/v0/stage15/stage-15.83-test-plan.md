# Stage 15.83 — Test Plan: AggregateKind (Array + Adt) Span Accuracy Fix

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.207.0 → v0.208.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.83 fixes the last 2 `Span::DUMMY` error sites in
`infer_rvalue`: the `AggregateKind::Array` and `AggregateKind::Adt`
unify errors.

## 2. New Integration Tests (2 tests)

Added to `tests/v0/stage15/plan/error_system_cleanup_tests.rs`:

### 2.1 `stage15_83_array_element_mismatch_span_points_to_array`

Tests that `[1, true, 3]` (array element type mismatch) produces a
typeck error whose span points to the array literal (byte offset >= 15),
not `1:1`.

```rust
let src = "fn main() { let x = [1, true, 3]; }";
let result = compile(src);
let mismatch_err = result.errors.typeck.iter()
    .find(|e| e.message.contains("mismatched types"))
    .expect("expected mismatched types error");
assert_ne!(mismatch_err.span.lo, 0, "span should not be Span::DUMMY");
assert!(mismatch_err.span.lo >= 15, "span should point into the statement (>= 15)");
```

### 2.2 `stage15_83_struct_field_mismatch_span_points_to_literal`

Tests that `S { x: true }` (struct field type mismatch) produces a
typeck error whose span points to the struct literal (byte offset >= 40),
not `1:1`.

```rust
let src = "struct S { x: i32 } fn main() { let s = S { x: true }; }";
let result = compile(src);
let mismatch_err = result.errors.typeck.iter()
    .find(|e| e.message.contains("mismatched types"))
    .expect("expected mismatched types error");
assert_ne!(mismatch_err.span.lo, 0, "span should not be Span::DUMMY");
assert!(mismatch_err.span.lo >= 40, "span should point into the struct literal (>= 40)");
```

## 3. Conformance Test Impact

### 3.1 No conformance test changes

All 5216 conformance tests pass unchanged. The span accuracy changes
don't affect `ERROR_PATTERN` matching because patterns check for
substrings in the error message, not the span location.

## 4. Manual Verification

Verified the improved error spans manually:

### 4.1 `[1, true, 3]` — span points to the array

```
$ echo 'fn main() { let x = [1, true, 3]; }' | landin-stage0 --compile
error[E400]: mismatched types: expected bool, found {integer}
  --> /tmp/t.lin:1:21
  |
1 | fn main() { let x = [1, true, 3]; }
  |                     ^

note: expected: bool
  --> /tmp/t.lin:1:21
  |
1 | fn main() { let x = [1, true, 3]; }
  |                     ^

note: found: {integer}
  --> /tmp/t.lin:1:21
  |
1 | fn main() { let x = [1, true, 3]; }
  |                     ^
```

**Before**: `--> /tmp/t.lin:1:1` (file start, useless)
**After**: `--> /tmp/t.lin:1:21` (the array literal, with snippet underline)

### 4.2 `S { x: true }` — span points to the struct literal

```
$ echo 'struct S { x: i32 } fn main() { let s = S { x: true }; }' | landin-stage0 --compile
error[E400]: mismatched types: expected bool, found i32
  --> /tmp/t2.lin:1:41
  |
1 | struct S { x: i32 } fn main() { let s = S { x: true }; }
  |                                         ^

note: expected: bool
  --> /tmp/t2.lin:1:41
  |
1 | struct S { x: i32 } fn main() { let s = S { x: true }; }
  |                                         ^

note: found: i32
  --> /tmp/t2.lin:1:41
  |
1 | struct S { x: i32 } fn main() { let s = S { x: true }; }
  |                                         ^
```

**Before**: `--> /tmp/t2.lin:1:1` (file start, useless)
**After**: `--> /tmp/t2.lin:1:41` (the struct literal, with snippet underline)

## 5. Regression Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Conformance `ERROR_PATTERN` matches break | LOW | Patterns check message substrings, not span locations |
| Rust integration tests check span values | LOW | No existing tests assert on Span::DUMMY; the 2 new tests assert on the new accurate spans |
| Span override produces wrong location | LOW | 2 new tests verify span is in the statement range; manual verification confirms snippet underlines the right token |

**Overall risk**: LOW. The changes are localized to 2 error sites in
`infer_rvalue`. The 2 new integration tests verify the exact behavior
for Array and Adt aggregate paths.

## 6. Acceptance Criteria (§9.1)

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 232/232 PASS | ✅ 232/232 PASS |
| `cargo test --features llvm-backend --test all_tests` | 2140/2140 PASS | ✅ 2140/2140 PASS (was 2138, +2 new) |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |
| Conformance test count | unchanged (5216) | ✅ 5216 |

## 7. Test Sign-off

- ✅ All 232 lib tests pass
- ✅ All 2140 integration tests pass (was 2138, +2 new span accuracy tests)
- ✅ All 5216 conformance tests pass
- ✅ 0 conformance test flips
- ✅ 0 clippy warnings
- ✅ fmt clean
- ✅ Manual verification: error spans now point to actual aggregate literals

**Stage 15.83 PASSED**.
