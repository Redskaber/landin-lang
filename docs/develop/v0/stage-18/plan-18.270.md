# Stage 18.270 — TD-GENERIC-FN-RETURN-EXPECTED-TY Complete Fix (Block expected_ty propagation)

> **Author**: Super Z (main) — Stage Committee (ARCH-A + DEV-A + QA-A)
> **Date**: 2026-08-25
> **Version**: v0.492.0 (no bump — soundness fix)
> **Process**: stage-committee-process.md v6.4 §17.6 (缺陷纳入 — "直到审查不出问题为止")
> **Status**: ✅ Complete — soundness hole FULLY CLOSED

---

## 1. Executive Summary

This stage completes the fix for TD-GENERIC-FN-RETURN-EXPECTED-TY.
Stage 18.269 implemented Phase 2d (threading `expected_ty = return_mir_ty`
into body tail expression), but it was incomplete because the fn body
is a Block, and the Block arm in `lower_expr_to_operand` didn't pass
`expected_ty` to `lower_block`. This stage fixes that by adding
`expected_ty: Option<&Ty>` param to `lower_block`.

### 1.1 Outcomes

| Aspect | Value |
|--------|-------|
| New soundness holes closed | 1 (TD-GENERIC-FN-RETURN-EXPECTED-TY) |
| New regression tests | 5 (2 positive + 3 negative) |
| Test count | 3900 (was 3895), 0 failures |
| Files modified | 3 (`mir/lower/control_flow.rs`, `mir/lower/expr_operand.rs`, `mir/lower/expr_variants.rs`) |

### 1.2 Verification

- ✅ `cargo build --features llvm-backend` — 0 warnings
- ✅ `cargo check --features llvm-backend` — 0 errors, 0 warnings
- ✅ `cargo fmt --check` — 0 diff
- ✅ `cargo clippy --all-targets --features llvm-backend -- -D warnings` — 0 warnings
- ✅ `cargo test --features llvm-backend` — 3900 tests, 0 failures

---

## 2. Root Cause + Fix

### 2.1 Root Cause

The fn body `Holder(true)` in `fn make() -> Holder<i32> { Holder(true) }`
is a Block expression `{ Holder(true) }`. The call chain was:

1. `body_lower.rs`: `lower_expr_to_operand(&body.value, expected_ty)` —
   correctly passes `expected_ty = Some(Adt(DefId(0), [Int(I32)]))`
2. `expr_operand.rs` Block arm: `control_flow::lower_block(cx, block)` —
   **DOESN'T pass expected_ty!** (was `None`)
3. `control_flow.rs` lower_block: `lower_expr_to_operand(cx, expr, None)` —
   trailing expression gets `None`

So the expected_ty was computed in body_lower.rs but lost at step 2.

### 2.2 Fix

1. Added `expected_ty: Option<&Ty>` param to `lower_block` in `control_flow.rs`
2. Threaded `expected_ty` into the trailing expression's `lower_expr_to_operand`
3. Updated Block arm in `lower_expr_to_operand` to pass `expected_ty`
4. Updated all other callers to pass `None` (they're not in expected_ty context)

---

## 3. Committee Voting

| Role | Vote | Reason |
|------|------|--------|
| ARCH-A | APPROVED | Root cause identified; fix is correct + minimal |
| DEV-A | APPROVED | ~20 LOC change; mechanical param threading |
| QA-A | APPROVED | 5 regression tests verify; ratio met |

**Result: 3/3 APPROVED**

---

## 4. References

- Stage 18.269 plan: `docs/develop/v0/stage-18/plan-18.269.md`
- Tech-debt-register: `docs/develop/v0/tech-debt-register.md` (TD-GENERIC-FN-RETURN-EXPECTED-TY ✅)
- Regression tests: `tests/v0/stage18/plan/stage18_270_fn_return_expected_ty_regression_tests.rs`
