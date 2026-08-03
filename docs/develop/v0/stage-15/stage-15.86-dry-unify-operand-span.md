# Stage 15.86 — DRY Refactor: Unify operand_span into mir::place

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.210.0 → v0.211.0
> **Process**: stage-committee-process.md v3.24 §29 + §9.3 + §25 + §14.4

## 1. Executive Summary

Stage 15.86 is a DRY (Don't Repeat Yourself) refactor that unifies the
two duplicate `operand_span` helpers into a single shared function in
`mir::place`. This follows §23 rule 5 (DRY) and §1.0 原則 5 "去除兼容思维".

**Background**: Stages 15.81 and 15.85 each added a private
`operand_span` method to `TypeChecker` and `BorrowChecker` respectively.
The two methods were identical (same signature, same body, same doc
comment). Per §23 rule 5, this duplication should be eliminated.

**Fix**:
1. Added `pub fn operand_span(op: &Operand) -> Span` to `mir::place`
   (the module that defines `Operand`).
2. Removed the private `operand_span` method from `TypeChecker`
   (`src/typeck/checker.rs`).
3. Removed the private `operand_span` method from `BorrowChecker`
   (`src/borrowck/mod.rs`).
4. Updated all 8 callers (4 in typeck, 4 in borrowck) to use
   `crate::mir::place::operand_span(op)`.
5. Updated the existing borrowck unit test to call the shared function.
6. Added a new unit test in `mir::place::tests` for the shared function.

**Test impact**:
- 1 new Rust unit test in `mir::place::tests` (for the shared helper)
- 0 conformance test changes
- **Total: 7591 tests passing** (235 lib [was 234, +1 new] + 2140
  integration + 5216 conformance), 0 failures, 0 warnings.

Per §1.0 原則 5 "去除兼容思维": duplicate code removed.
Per §23 rule 5 (DRY): single source of truth for operand span extraction.

## 2. Why This Matters

Code duplication is a maintenance liability:
- If the `operand_span` logic needs to change (e.g., adding span support
  for `Const`), both copies must be updated in sync.
- The two copies were identical, but there was no guarantee they would
  stay identical — a future change to one might not propagate to the
  other.
- The duplication was explicitly noted in Stage 15.85's doc comment:
  "a future stage can unify these into a shared `mir::place` helper."

This stage fulfills that note. The shared helper lives in `mir::place`
(the module that defines `Operand`), which is the architecturally
correct location per §16 (interface isolation): the span extraction
logic is a property of `Operand`, not of `TypeChecker` or
`BorrowChecker`.

## 3. The Refactor

### 3.1 New shared helper (`src/mir/place.rs`)

```rust
/// Stage 15.86: Extract the source span from an `Operand`.
///
/// Shared helper used by both `typeck::checker::TypeChecker` and
/// `borrowck::mod::BorrowChecker` to attach accurate spans to errors
/// that originate from operand checks.
pub fn operand_span(op: &Operand) -> Span {
    match op {
        Operand::Copy(lv) | Operand::Move(lv) => lv.span,
        Operand::Constant(_) => Span::DUMMY,
    }
}
```

### 3.2 Removed private methods

**`src/typeck/checker.rs`** — removed:
```rust
fn operand_span(op: &Operand) -> Span { ... }  // was on TypeChecker
```

**`src/borrowck/mod.rs`** — removed:
```rust
fn operand_span(op: &Operand) -> Span { ... }  // was on BorrowChecker
```

### 3.3 Updated callers (8 sites)

**typeck** (4 sites in `check_terminator` + `post_check_terminator`):
```rust
// Before
Self::operand_span(func)
Self::operand_span(discr)
Self::operand_span(cond)

// After
crate::mir::place::operand_span(func)
crate::mir::place::operand_span(discr)
crate::mir::place::operand_span(cond)
```

**borrowck** (4 sites in `check_terminator`):
```rust
// Before
Self::operand_span(func)
Self::operand_span(arg)
Self::operand_span(discr)
Self::operand_span(cond)

// After
crate::mir::place::operand_span(func)
crate::mir::place::operand_span(arg)
crate::mir::place::operand_span(discr)
crate::mir::place::operand_span(cond)
```

### 3.4 Updated existing test

The borrowck test `stage15_85_operand_span_extracts_place_span` was
updated to call the shared function:
```rust
// Before
assert_eq!(BorrowChecker::operand_span(&copy_op), span);

// After
use crate::mir::place::operand_span;
assert_eq!(operand_span(&copy_op), span);
```

### 3.5 New unit test in mir::place

```rust
/// Stage 15.86: Verify the shared `operand_span` helper.
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

## 4. API Naming Compliance (§23)

**New public function**:

| Function | Location | §23 Compliance |
|----------|----------|-----------------|
| `operand_span(op: &Operand) -> Span` | `mir::place` | ✅ `<noun>_<noun>` (property accessor — no `get_` prefix) |

Per §23 rule 5 (DRY): single source of truth. The two private duplicates
are removed; the shared helper is the sole definition.

Per §23 rule 2 (context type naming): the function is a free function
in `mir::place`, not a method on a struct. This is appropriate because
it's a pure property accessor on `Operand` (an enum), not a stateful
operation requiring a context.

## 5. §16 Interface Isolation

The shared `operand_span` lives in `mir::place` (same module as
`Operand`). It reads only `Operand` data (via `Place.span`) — no HIR
lookup, no resolver access, no typeck/borrowck access.

Callers import via `crate::mir::place::operand_span`. This is a clean
dependency: `typeck` and `borrowck` both depend on `mir::place` (they
already use `Operand`, `Place`, etc.). No new cross-stage dependencies.

## 6. §25 Deep Review (8 Dimensions)

| Dimension | Status | Notes |
|-----------|--------|-------|
| D1 Architecture | ✅ | Helper in `mir::place` (same module as `Operand`); callers import explicitly |
| D2 Tech Debt | ✅ | 1 duplicate eliminated (2 copies → 1 shared) |
| D3 Test Coverage | ✅ | 1 new unit test in `mir::place`; existing borrowck test updated |
| D4 Next-Phase Readiness | ✅ | No regressions; DRY-compliant |
| D5 Design Rationality | ✅ | Span extraction is a property of `Operand`, not of `TypeChecker`/`BorrowChecker` |
| D6 Performance | ✅ | No change (same code, different location) |
| D7 Documentation | ✅ | This doc + test plan + matrix updated |
| D8 Test Path Coverage | ✅ | Shared helper has unit test; all 8 callers verified by existing conformance tests |

**Committee Vote**: GO — Stage 15.86 complete.

## 7. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 235/235 PASS (was 234, +1 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2140/2140 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
- **Total: 7591 tests passing, 0 failures, 0 warnings.**

## 8. Refactor Summary

This is a pure refactor — no behavior change. The error spans produced
by typeck and borrowck are identical before and after this stage. The
only difference is that the `operand_span` logic now lives in one
place (`mir::place`) instead of two (private methods on `TypeChecker`
and `BorrowChecker`).

Per §14.4 (重构即架构设计): the refactor reorganizes the code to
reflect the architectural truth — span extraction is a property of
`Operand`, not of the checker that uses it.

## 9. Next Steps

The error system cleanup (Stages 15.80-15.85) + DRY refactor
(Stage 15.86) is complete. The next major v0.2 task is:

**Task 12 (Lifetime elision)** — the next major v0.2 task (2-3 weeks,
P1, ready now). This is the last remaining P1 task for v0.2 release.

## 10. Version Policy

v0.210.0 → v0.211.0 (minor bump — Phase 2 DRY refactor + 1 new unit test).
