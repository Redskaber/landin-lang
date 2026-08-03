# Stage 15.85 — Test Plan: Borrowck check_terminator Span Accuracy Fix

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.209.0 → v0.210.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.85 fixes 4 `Span::DUMMY` error sites in
`borrowck::mod::check_terminator`:
1. `Call { func, .. }` — func operand
2. `Call { args, .. }` — each arg operand
3. `SwitchInt { discr, .. }` — discr operand
4. `Assert { cond, .. }` — cond operand

All 4 sites now use `Self::operand_span(op)` instead of `Span::DUMMY`.

## 2. New Unit Test (1 test)

Added to `src/borrowck/mod.rs::tests`:

### 2.1 `stage15_85_operand_span_extracts_place_span`

Tests that `operand_span` correctly extracts the Place span from
Copy/Move operands and returns DUMMY for Constant:

```rust
#[test]
fn stage15_85_operand_span_extracts_place_span() {
    let span = Span::new(42, 45);
    let place = Place::local(LocalId(0), span);
    // Copy operand → returns the place's span.
    let copy_op = Operand::Copy(place.clone());
    assert_eq!(BorrowChecker::operand_span(&copy_op), span);
    // Move operand → returns the place's span.
    let move_op = Operand::Move(place);
    assert_eq!(BorrowChecker::operand_span(&move_op), span);
    // Constant operand → returns Span::DUMMY (Const has no span field).
    let const_op = Operand::Constant(crate::mir::ty::Const {
        ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
        val: crate::mir::ty::ConstVal::Int(42),
    });
    assert_eq!(BorrowChecker::operand_span(&const_op), Span::DUMMY);
}
```

## 3. Conformance Test Impact

### 3.1 No conformance test changes

All 5216 conformance tests pass unchanged. The span accuracy changes
don't affect `ERROR_PATTERN` matching because:
- `ERROR_PATTERN` checks for substrings in the error message (e.g.,
  "moved", "Copy"), not the span location.
- The error messages themselves are unchanged (only the span is fixed).
- The use-after-move and not-Copy errors require resolver-backed Copy
  detection (not available in test contexts where Adt is treated as
  Copy), so these errors are rare in conformance tests.

## 4. Pipeline Path Coverage (§17.5.1)

### 4.1 Call terminator use-after-move path (with accurate span)

| Stage | Path | Test |
|-------|------|------|
| Lexer | (no change) | ✅ |
| Parser | (no change) | ✅ |
| HIR lower | (no change) | ✅ |
| MIR lower | `Call` terminator with func/args operands (Place has span) | ✅ |
| Typeck | (no change) | ✅ |
| Borrowck | `check_terminator` Call arm uses `Self::operand_span(func/arg)` | ✅ `stage15_85_operand_span_extracts_place_span` (unit test for helper) |
| Driver | `to_diagnostics` renders the accurate span | ✅ Existing conformance tests pass |

### 4.2 SwitchInt/Assert terminator paths (with accurate span)

Same pattern — `Self::operand_span(discr/cond)` instead of `Span::DUMMY`.

## 5. Manual Verification

The use-after-move and not-Copy errors require resolver-backed Copy
detection (not available in test contexts where Adt is treated as
Copy). These errors are verified indirectly:

- **`operand_span` unit test**: verifies the helper extracts Place.span
  for Copy/Move and returns DUMMY for Constant.
- **Existing conformance tests**: all 5216 pass, confirming the span
  changes don't break `ERROR_PATTERN` matching.
- **Code review**: the 4 `check_terminator` sites now use
  `Self::operand_span(op)` instead of `Span::DUMMY`, matching the
  pattern established in typeck Stage 15.81.

## 6. Regression Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Conformance `ERROR_PATTERN` matches break | LOW | Patterns check message substrings, not span locations |
| `operand_span` returns wrong span | LOW | 1 unit test verifies exact behavior for Copy/Move/Constant |
| Span produces wrong location in errors | LOW | Helper extracts `Place.span` directly; no transformation |
| Use-after-move errors become harder to debug | LOW | Accurate spans make them easier to locate (was: "1:1") |

**Overall risk**: LOW. The changes are localized to 4 error span sites
in `check_terminator`. The 1 new unit test verifies the new helper.

## 7. Acceptance Criteria (§9.1)

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 234/234 PASS | ✅ 234/234 PASS (was 233, +1 new) |
| `cargo test --features llvm-backend --test all_tests` | 2140/2140 PASS | ✅ 2140/2140 PASS |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |
| Conformance test count | unchanged (5216) | ✅ 5216 |

## 8. Test Sign-off

- ✅ All 234 lib tests pass (was 233, +1 new `operand_span` test)
- ✅ All 2140 integration tests pass
- ✅ All 5216 conformance tests pass
- ✅ 0 conformance test flips
- ✅ 0 clippy warnings
- ✅ fmt clean

**Stage 15.85 PASSED**.
