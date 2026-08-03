# Stage 15.83 — AggregateKind (Array + Adt) Span Accuracy Fix

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.207.0 → v0.208.0
> **Process**: stage-committee-process.md v3.24 §29 + §9.3 + §25

## 1. Executive Summary

Stage 15.83 fixes the last 2 `Span::DUMMY` error sites in `infer_rvalue`:
the `AggregateKind::Array` and `AggregateKind::Adt` unify errors. These
errors occur when array element types or struct field types mismatch
(e.g., `[1, true, 3]`, `S { x: true }` where `x` is `i32`).

**Fix**: Override the unify error span with `stmt_span` (from Stage
15.82's `infer_rvalue` parameter) at both call sites.

**Before** (`[1, true, 3]`):
```
error[E400]: mismatched types: expected bool, found {integer}
  --> /tmp/t.lin:1:1

note: expected: bool
  --> /tmp/t.lin:1:1

note: found: {integer}
  --> /tmp/t.lin:1:1
```

**After**:
```
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

**Test impact**:
- 2 new Rust integration tests for span accuracy
- 0 conformance test changes (all `ERROR_PATTERN` matches preserved)
- **Total: 7588 tests passing** (232 lib + 2140 integration [was 2138,
  +2 new] + 5216 conformance), 0 failures, 0 warnings.

Per §1.0 原則 4 "报错 > 静默": error locations are accurate, not cryptic.

## 2. Why This Matters

The `AggregateKind::Array` and `AggregateKind::Adt` unify errors are
the most common type mismatch errors users encounter:
- Array literals with mixed element types (`[1, true, 3]`)
- Struct literals with wrong field types (`S { x: true }` where `x`
  is `i32`)

Previously, these errors all showed "1:1" (file start) because the
unify error span wasn't overridden. This made the errors hard to
locate — the user would see "1:1" and have to search the file.

The fix uses `stmt_span` (threaded through `infer_rvalue` in Stage
15.82) to override the unify error span. Now both error types point
to the actual aggregate literal.

## 3. The Fix

### 3.1 `AggregateKind::Array` unify error

```rust
// Before
for op in operands {
    let op_ty = self.infer_operand(mir, op);
    if let Err(e) = self.unify.unify(&op_ty, elem_ty) {
        self.errors.push(*e);  // e.span is Span::DUMMY from mismatch()
    }
}

// After
for op in operands {
    let op_ty = self.infer_operand(mir, op);
    if let Err(mut e) = self.unify.unify(&op_ty, elem_ty) {
        if stmt_span != Span::DUMMY {
            e.span = stmt_span;  // override with actual span
        }
        self.errors.push(*e);
    }
}
```

### 3.2 `AggregateKind::Adt` unify error

Same pattern — override the unify error span with `stmt_span`:

```rust
// Before
if let Err(e) = self.unify.unify(&op_ty, field_ty) {
    self.errors.push(*e);
}

// After
if let Err(mut e) = self.unify.unify(&op_ty, field_ty) {
    if stmt_span != Span::DUMMY {
        e.span = stmt_span;
    }
    self.errors.push(*e);
}
```

## 4. API Naming Compliance (§23)

No API surface changes. The fix modifies 2 error sites inside
`infer_rvalue` (private method). No new public functions, types, or
re-exports.

## 5. §16 Interface Isolation

The fix is entirely within `typeck::checker::infer_rvalue`. It uses
`stmt_span` (already available from Stage 15.82) and `self.unify`
(already available). No cross-stage access.

## 6. §25 Deep Review (8 Dimensions)

| Dimension | Status | Notes |
|-----------|--------|-------|
| D1 Architecture | ✅ | Follows the span-override pattern from Stages 15.81-15.82 |
| D2 Tech Debt | ✅ | 2 more `Span::DUMMY` error sites fixed (last in `infer_rvalue`) |
| D3 Test Coverage | ✅ | 2 new integration tests verify span accuracy |
| D4 Next-Phase Readiness | ✅ | No regressions; all aggregate errors now have accurate spans |
| D5 Design Rationality | ✅ | Consistent with the span-override pattern |
| D6 Performance | ✅ | One extra `if` per aggregate element; negligible |
| D7 Documentation | ✅ | This doc + test plan + matrix updated |
| D8 Test Path Coverage | ✅ | Array + Adt aggregate paths have span accuracy tests |

**Committee Vote**: GO — Stage 15.83 complete.

## 7. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 232/232 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2140/2140 PASS (was 2138, +2 new)
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
- **Total: 7588 tests passing, 0 failures, 0 warnings.**

## 8. Error System Cleanup Summary (Stages 15.80-15.83)

The four-stage error system cleanup is now complete:

| Stage | Focus | Sites Fixed |
|-------|-------|-------------|
| 15.80 | Human-readable type names (`type_to_string`) | 6 `{:?}` leaks + 2 `({:?})` enum leaks |
| 15.81 | Terminator span accuracy (`operand_span`, `term.span`) | 7 `Span::DUMMY` sites + 1 `{:?}` leak |
| 15.82 | Statement/rvalue span accuracy (`stmt_span` in `infer_rvalue`) | 9 `Span::DUMMY` sites + 5 `{:?}` leaks |
| 15.83 | Aggregate (Array + Adt) span accuracy | 2 `Span::DUMMY` sites |
| **Total** | | **20 `Span::DUMMY` sites + 14 `{:?}` leaks fixed** |

**Result**: All user-facing typeck error messages now:
- Use human-readable type names (`i32`, `bool`, `&mut T`, etc.) — no
  Debug format leaks
- Point to actual source locations (with snippet underlines) — no
  `Span::DUMMY` / "1:1" errors

The error system is now in good shape for user-facing work.

## 9. Remaining Span::DUMMY Sites (deferred)

The following `Span::DUMMY` sites remain but are lower priority:

- `Const` struct has no `span` field — `operand_span` returns
  `Span::DUMMY` for `Operand::Constant`. Adding a `span` field to
  `Const` would break `Eq + Hash` derives (Span doesn't implement
  those). Deferred to a future stage (requires Span to implement
  Eq+Hash, or a different design).
- `Ty::new(...)` calls inside `infer_rvalue` use `Span::DUMMY` for the
  `Ty` span — but `Ty` no longer has a span field (removed in Stage
  15.5 for interning). These `Span::DUMMY` arguments are ignored (the
  `Ty::new` constructor accepts a span for backward compatibility but
  doesn't store it).

These don't affect user-facing error messages.

## 10. Next Steps

The error system cleanup is complete. The next major v0.2 task is:

**Task 12 (Lifetime elision)** — the next major v0.2 task (2-3 weeks,
P1, ready now). This is the last remaining P1 task for v0.2 release.

## 11. Version Policy

v0.207.0 → v0.208.0 (minor bump — Phase 2 error system span accuracy
fix + 2 new integration tests).
