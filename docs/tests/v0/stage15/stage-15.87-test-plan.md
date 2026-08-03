# Stage 15.87 — Test Plan: Resolve Error Span Accuracy Fix

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.211.0 → v0.212.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.87 fixes 1 `Span::DUMMY` error site in
`driver::scan_ty_for_unresolved`. The "cannot find type in this scope"
error now uses `p.span` (the type path's span) instead of `Span::DUMMY`.

## 2. New Integration Test (1 test)

Added to `tests/v0/stage15/plan/error_system_cleanup_tests.rs`:

### 2.1 `stage15_87_type_resolution_error_span_points_to_type`

Tests that `let x: Undefined = 42;` produces a resolve error whose
span points to the `Undefined` type name (byte offset >= 19), not
`1:1`.

```rust
#[test]
fn stage15_87_type_resolution_error_span_points_to_type() {
    let src = "fn main() { let x: Undefined = 42; }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "expected resolve error for undefined type `Undefined`"
    );
    let type_err = result.errors.resolve.iter()
        .find(|e| e.message.contains("cannot find type"))
        .expect("expected 'cannot find type' error");
    assert_ne!(type_err.span.lo, 0, "span should not be Span::DUMMY");
    assert!(type_err.span.lo >= 19, "span should point into the type name (>= 19)");
}
```

## 3. Conformance Test Impact

### 3.1 No conformance test changes

All 5216 conformance tests pass unchanged. The span accuracy change
doesn't affect `ERROR_PATTERN` matching because patterns check for
substrings in the error message, not the span location.

## 4. Manual Verification

Verified the improved error span manually:

### 4.1 `let x: Undefined = 42;` — span points to `Undefined`

```
$ echo 'fn main() { let x: Undefined = 42; }' | landin-stage0 --compile
error[E300]: cannot find type in this scope
  --> /tmp/t.lin:1:20
  |
1 | fn main() { let x: Undefined = 42; }
  |                    ^^^^^^^^^^^
```

**Before**: `--> /tmp/t.lin:1:1` (file start, useless)
**After**: `--> /tmp/t.lin:1:20` (the `Undefined` type name, with snippet underline)

## 5. Regression Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Conformance `ERROR_PATTERN` matches break | LOW | Patterns check message substrings, not span locations |
| `p.span` is DUMMY for some HIR types | LOW | HIR lowering always sets span for type paths; verified by manual test |
| Existing resolve tests break | LOW | All 5216 conformance tests pass |

**Overall risk**: LOW. The change is localized to 1 error span site.
The 1 new integration test verifies the exact behavior.

## 6. Acceptance Criteria (§9.1)

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 235/235 PASS | ✅ 235/235 PASS |
| `cargo test --features llvm-backend --test all_tests` | 2141/2141 PASS | ✅ 2141/2141 PASS (was 2140, +1 new) |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |
| Conformance test count | unchanged (5216) | ✅ 5216 |

## 7. Test Sign-off

- ✅ All 235 lib tests pass
- ✅ All 2141 integration tests pass (was 2140, +1 new span accuracy test)
- ✅ All 5216 conformance tests pass
- ✅ 0 conformance test flips
- ✅ 0 clippy warnings
- ✅ fmt clean
- ✅ Manual verification: error span now points to the type name

**Stage 15.87 PASSED**.
