# Stage 15.86 — Test Plan: DRY Refactor (Unify operand_span)

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.210.0 → v0.211.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.86 is a pure DRY refactor — no behavior change. The two
duplicate `operand_span` private methods (on `TypeChecker` and
`BorrowChecker`) are replaced by a single shared `pub fn operand_span`
in `mir::place`.

## 2. New Unit Test (1 test)

Added to `src/mir/place.rs::tests`:

### 2.1 `stage15_86_operand_span_extracts_place_span`

Tests that the shared `operand_span` helper correctly extracts the
Place span from Copy/Move operands and returns DUMMY for Constant:

```rust
#[test]
fn stage15_86_operand_span_extracts_place_span() {
    let span = Span::new(42, 45);
    let place = Place::local(LocalId(0), span);
    let copy_op = Operand::Copy(place.clone());
    assert_eq!(operand_span(&copy_op), span);
    let move_op = Operand::Move(place);
    assert_eq!(operand_span(&move_op), span);
    let const_op = Operand::Constant(Const {
        ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
        val: ConstVal::Int(42),
    });
    assert_eq!(operand_span(&const_op), Span::DUMMY);
}
```

## 3. Updated Existing Test (1 test)

The borrowck test `stage15_85_operand_span_extracts_place_span` was
updated to call the shared function instead of the (now-removed)
`BorrowChecker::operand_span`:

```rust
// Before
assert_eq!(BorrowChecker::operand_span(&copy_op), span);

// After
use crate::mir::place::operand_span;
assert_eq!(operand_span(&copy_op), span);
```

## 4. Conformance Test Impact

### 4.1 No conformance test changes

All 5216 conformance tests pass unchanged. This is expected because
the refactor is pure — no behavior change. The error spans produced
by typeck and borrowck are identical before and after.

## 5. Pipeline Path Coverage (§17.5.1)

### 5.1 Shared operand_span path

| Stage | Path | Test |
|-------|------|------|
| `mir::place` | `operand_span(op)` extracts Place.span | ✅ `stage15_86_operand_span_extracts_place_span` |
| `typeck::checker` | 4 callers use `crate::mir::place::operand_span` | ✅ Existing conformance tests (Stages 15.81-15.83 span tests) |
| `borrowck::mod` | 4 callers use `crate::mir::place::operand_span` | ✅ Existing conformance tests + `stage15_85_operand_span_extracts_place_span` |

## 6. Regression Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Behavior change from refactor | LOW | The shared function body is identical to the two private copies; verified by diff |
| Import path errors | LOW | All 8 callers updated; build passes |
| Existing span tests break | LOW | All 5216 conformance + 2140 integration tests pass |
| `operand_span` visibility issue | LOW | `pub fn` in `mir::place`; accessible from all callers |

**Overall risk**: LOW. The refactor is pure — same code, different
location. The 1 new unit test + existing tests verify correctness.

## 7. Acceptance Criteria (§9.1)

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 235/235 PASS | ✅ 235/235 PASS (was 234, +1 new) |
| `cargo test --features llvm-backend --test all_tests` | 2140/2140 PASS | ✅ 2140/2140 PASS |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |
| Conformance test count | unchanged (5216) | ✅ 5216 |

## 8. Test Sign-off

- ✅ All 235 lib tests pass (was 234, +1 new `operand_span` test in `mir::place`)
- ✅ All 2140 integration tests pass
- ✅ All 5216 conformance tests pass
- ✅ 0 conformance test flips
- ✅ 0 clippy warnings
- ✅ fmt clean
- ✅ No behavior change (pure refactor)

**Stage 15.86 PASSED**.
