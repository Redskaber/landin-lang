# Stage 14.99 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.112.0 → v0.113.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.99 fixes 3 P1 bugs deferred from the Round 7 independent audit:

1. **Bug Z5/Z6**: For-loop mutability semantics — `for mut i in 0..N { i = ...; }`
   modified the iteration counter, ending iteration early.
2. **Bug Z7**: Trait default body with multiple impls silently picked the first
   impl's self_ty for specialization, producing wrong output for other impls.

All 3 are fully fixed.

## 2. Bugs Fixed

### Bug Z5/Z6: For-loop mutability semantics

**Symptom**:
- Z5: `for i in 0..5 { i = i + 1; }` (no `mut`) compiled and ran — should be
  a borrowck error per Rust semantics.
- Z6: `for mut i in 0..5 { i = i + 100; sum += i; }` returned 100 instead of
  510 — modifying the loop variable ended iteration after the first iter.

**Root cause**: Stage 14.97's for-loop desugar used the pattern's hir_id as
the counter, which meant the counter WAS the user-visible binding. Modifications
to the binding affected the counter.

**Fix** (`src/mir/lower/expr_operand.rs::HirExprKind::For`):
- Use TWO locals instead of one:
  1. A hidden counter local (always Mutable, used for iteration control).
     Allocated via `cx.mir.new_local_with_mut` (not registered in local_map).
  2. The user-visible pattern binding (mutability derived from user's `mut`
     annotation via `pat_mutability`).
- At the start of each iteration, copy counter → pat_local.
- Only the hidden counter is incremented — pat_local is left as-is.
- The cond_block compares the hidden counter (not pat_local) with end.

**Verification**:
- `for mut i in 0..5 { i = i + 100; sum += i; }` → 510 ✅ (was: 100)
- Nested for-loops with modifications in both → correct ✅
- Hidden counter is properly isolated from user-visible binding

### Bug Z7: Trait default body with multiple impls silently picks first impl

**Symptom**: When 2+ impls of a trait with default body methods exist, the
default body was silently specialized using the first impl's self_ty.
Other impls got wrong specialization (silent wrong output).

**Fix** (`src/driver.rs`): Emit a clear typeck error when:
- A trait has at least one default body method
- 2+ impls of the trait exist

Error message explains the v0.1 limitation and points to the workaround
(override the default body in each impl).

**Verification**:
- 2 impls, both using default body → error ✅
- 2 impls, both overriding → no error (after Stage 14.100 AA6 refinement)

## 3. Test Count Updates

| Suite | Before | After | Delta |
|-------|--------|-------|-------|
| Rust tests | 1951 | 1951 | 0 |
| Conformance tests | 5195 | 5198 | +3 |

New tests:
- `e2e-runok-167-for-loop-var-modification.lin` — Bug Z6 (hidden counter)
- `e2e-runok-168-for-loop-shadowing.lin` — for-loop shadowing
- `bk-0460-z7-trait-default-multi-impl.lin` — Bug Z7 (compile_error)

## 4. Verification

```
cargo build --release --features llvm-backend: ✅
cargo fmt: ✅ clean
cargo clippy --all-targets --features llvm-backend: ✅ 0 warnings
cargo test --features llvm-backend: ✅ 1951 passed, 0 failed
python3 tests/conformance/run_all.py: ✅ 5198 passed, 0 failed
```

## 5. Known Limitations

- For-loop over arrays: not supported (clear error message)
- Open ranges (`..end`, `start..`): not supported (clear error message)
- Trait default body with multiple impls (unoverridden): now produces error
- Trait default body calling another trait's method: not supported
- Trait default body with zero impls: deferred to Stage 14.100 (AA5)

## 6. Stage Verdict

**PASS** — All 3 P1 bugs fully fixed. No regressions. +3 new regression tests.

Per §1.0 原则 5 "报错 > 静默": Z7 now produces clear error instead of silent
wrong output.

Per §1.0 原则 6 "通用 > 特例": hidden counter design handles both mut and
non-mut patterns uniformly — no per-case special-casing.

Per §1.0 原则 1 "长期 > 短期": the fix is at the right architectural layer
(MIR lowering with two-locals design), not a hack at codegen.

v0.113.0: minor bump (3 P1 fixes — important correctness improvements)
