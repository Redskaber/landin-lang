# Stage 15.88 — Test Plan: MIR Lowerer Debug Leak Fix

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.212.0 → v0.213.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.88 fixes 3 `{:?}` Debug format leaks in MIR lowering error
messages:
1. "no method found" error — `recv_ty.kind` (TyKind) Debug leak
2. "for-loop only supports Range" error — `iter.kind` (HirExprKind) Debug leak
3. "array repeat count" error — `count.kind` (HirExprKind) Debug leak

Also adds a new `hir_expr_kind_to_string` helper in `src/hir/kinds.rs`.

## 2. New Unit Test (1 test)

Added to `src/hir/kinds.rs::tests`:

### 2.1 `stage15_88_hir_expr_kind_to_string_basic`

Tests that `hir_expr_kind_to_string` produces human-readable labels for
common expression kinds:

```rust
#[test]
fn stage15_88_hir_expr_kind_to_string_basic() {
    let lit = HirExprKind::Lit(HirLitKind::Int(42, None));
    assert_eq!(hir_expr_kind_to_string(&lit), "literal");
    assert_eq!(hir_expr_kind_to_string(&HirExprKind::Unit), "unit");
    assert_eq!(hir_expr_kind_to_string(&HirExprKind::Continue), "continue");
    // ... Range, Tuple, Array, Call, MethodCall
}
```

## 3. New Integration Test (1 test)

Added to `tests/v0/stage15/plan/error_system_cleanup_tests.rs`:

### 3.1 `stage15_88_no_method_found_uses_human_readable_type_name`

Tests that `s.f()` (where `S` has no method `f`) produces an error
message with human-readable type name, not Debug format:

```rust
#[test]
fn stage15_88_no_method_found_uses_human_readable_type_name() {
    let src = "trait T { fn f(&self); } struct S; fn main() { let s = S; s.f(); }";
    let result = compile(src);
    let method_err = result.errors.typeck.iter()
        .find(|e| e.message.contains("no method"))
        .expect("expected 'no method' error");
    assert!(method_err.message.contains("<adt>"), "should contain '<adt>'");
    assert!(!method_err.message.contains("Adt("), "should NOT contain Debug 'Adt('");
    assert!(!method_err.message.contains("DefId("), "should NOT contain Debug 'DefId('");
}
```

## 4. Conformance Test Impact

### 4.1 No conformance test changes

All 5216 conformance tests pass unchanged. The Debug format fixes don't
affect `ERROR_PATTERN` matching because no conformance test checks for
Debug format in these messages.

## 5. Manual Verification

Verified the improved error messages manually:

### 5.1 "no method found" — human-readable type name

```
$ echo 'trait T { fn f(&self); } struct S; fn main() { let s = S; s.f(); }' | landin-stage0 --compile
error[E400]: no method `f` found for type `<adt>`
  --> /tmp/t.lin:1:59
  |
1 | trait T { fn f(&self); } struct S; fn main() { let s = S; s.f(); }
  |                                                           ^
```

**Before**: `no method `f` found for type `Adt(DefId(1), [])`` (Debug)
**After**: `no method `f` found for type `<adt>`` (human-readable)

## 6. Regression Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Conformance `ERROR_PATTERN` matches break | LOW | No patterns check for Debug format in these messages |
| `hir_expr_kind_to_string` returns wrong label | LOW | 1 unit test verifies 7 kinds (Lit, Unit, Continue, Range, Tuple, Array, Call, MethodCall) |
| New helper not exported | LOW | Added to `hir::mod` re-export; build passes |

**Overall risk**: LOW. The changes are localized to 3 error message
sites + 1 new helper. The 2 new tests verify the new behavior.

## 7. Acceptance Criteria (§9.1)

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 236/236 PASS | ✅ 236/236 PASS (was 235, +1 new) |
| `cargo test --features llvm-backend --test all_tests` | 2142/2142 PASS | ✅ 2142/2142 PASS (was 2141, +1 new) |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |
| Conformance test count | unchanged (5216) | ✅ 5216 |

## 8. Test Sign-off

- ✅ All 236 lib tests pass (was 235, +1 new `hir_expr_kind_to_string` test)
- ✅ All 2142 integration tests pass (was 2141, +1 new "no method found" test)
- ✅ All 5216 conformance tests pass
- ✅ 0 conformance test flips
- ✅ 0 clippy warnings
- ✅ fmt clean

**Stage 15.88 PASSED**.
