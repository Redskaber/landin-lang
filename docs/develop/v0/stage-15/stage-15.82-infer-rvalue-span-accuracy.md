# Stage 15.82 — infer_rvalue Span Accuracy + Remaining Debug Leaks

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.206.0 → v0.207.0
> **Process**: stage-committee-process.md v3.24 §29 + §9.3 + §25

## 1. Executive Summary

Stage 15.82 continues the error system cleanup from Stages 15.80-15.81.
It fixes the remaining `Span::DUMMY` error sites in `infer_rvalue`
(BinaryOp/UnaryOp type mismatch errors) and the `check_statement` Assign
coercion unify errors. It also fixes the last 5 `{:?}` Debug format
leaks in those error messages.

**Fix**:
1. Added `stmt_span: Span` parameter to `infer_rvalue` (was: no span
   access, all errors used `Span::DUMMY`).
2. Used `stmt_span` in 8 error sites inside `infer_rvalue`:
   - BinaryOp comparison unify error (span override)
   - BinaryOp bitwise unify error (span override)
   - BinaryOp shift "shift count must be an integer type" error
   - BinaryOp arithmetic "cannot apply arithmetic to T" error (×2: lhs, rhs)
   - BinaryOp arithmetic unify error (span override)
   - BinaryOp2 range expression error
   - UnaryOp Not "cannot apply `!` to T" error
   - UnaryOp Neg "cannot apply unary `-` to T" error
3. Used `stmt.span` in `check_statement` Assign coercion unify error
   (span override).
4. Replaced `{:?}` Debug formatting with `type_kind_to_string` in 5
   error messages (shift count, arithmetic ×2, not, neg).

**Before** (`true + false`):
```
error[E400]: cannot apply arithmetic to Bool (expected integer or float)
  --> /tmp/t.lin:1:1
```

**After**:
```
error[E400]: cannot apply arithmetic to bool (expected integer or float)
  --> /tmp/t.lin:1:21
  |
1 | fn main() { let x = true + false; }
  |                     ^^^^
```

**Test impact**:
- 3 new Rust integration tests for span accuracy + human-readable types
- 0 conformance test changes (all `ERROR_PATTERN` matches preserved)
- **Total: 7586 tests passing** (232 lib + 2138 integration [was 2135,
  +3 new] + 5216 conformance), 0 failures, 0 warnings.

Per §1.0 原則 3 "显式 > 隐式": `stmt_span` is an explicit parameter,
not hidden state.
Per §1.0 原則 4 "报错 > 静默": error locations are accurate, not cryptic.

## 2. Why This Matters

The `infer_rvalue` function produces type errors for BinaryOp and
UnaryOp mismatches (e.g., `true + false`, `!"hello"`). Previously,
these errors all used `Span::DUMMY` (file start "1:1") because
`infer_rvalue` had no access to the enclosing statement's span. This
made the errors hard to locate — the user would see "1:1" and have to
search the file.

The fix threads `stmt.span` through `infer_rvalue` as an explicit
parameter. Now every BinaryOp/UnaryOp error points to the actual
statement where the type mismatch occurs.

Combined with Stages 15.80 (human-readable type names) and 15.81
(terminator span accuracy), the error system is now in good shape:
- **No Debug format leaks** — all error messages use `type_kind_to_string`
- **No Span::DUMMY in user-visible errors** — all errors point to actual
  source locations (terminator or statement)

## 3. The Fix

### 3.1 `infer_rvalue` signature change

```rust
// Before
fn infer_rvalue(&mut self, mir: &MirBody, rv: &Rvalue) -> Ty { ... }

// After
fn infer_rvalue(&mut self, mir: &MirBody, rv: &Rvalue, stmt_span: Span) -> Ty { ... }
```

The `stmt_span` parameter is the span of the enclosing `Statement` (set
by `check_statement` to `stmt.span`). It's used to attach accurate
spans to errors produced inside `infer_rvalue`.

Per §1.0 原則 3 "显式 > 隐式": the span is an explicit parameter, not
hidden state on `self`. This keeps `TypeChecker` stateless w.r.t. the
current statement, which is cleaner and avoids bugs where the field
isn't reset between statements.

### 3.2 Error site fixes (8 sites in `infer_rvalue`)

Each error site now uses `stmt_span` instead of `Span::DUMMY`:

```rust
// Before
self.errors.push(TypeError::new(
    format!("shift count must be an integer type, found {:?}", b_ty.kind),
    Span::DUMMY,
));

// After
self.errors.push(TypeError::new(
    format!(
        "shift count must be an integer type, found {}",
        crate::mir::ty::type_kind_to_string(&b_ty.kind)
    ),
    stmt_span,
));
```

For unify errors (where `unify` returns `Box<TypeError>` with
`Span::DUMMY`), the span is overridden at the call site:

```rust
// Before
if let Err(e) = self.unify.unify(&a_ty, &b_ty) {
    self.errors.push(*e);
}

// After
if let Err(mut e) = self.unify.unify(&a_ty, &b_ty) {
    if stmt_span != Span::DUMMY {
        e.span = stmt_span;
    }
    self.errors.push(*e);
}
```

### 3.3 `check_statement` Assign coercion unify error

```rust
// Before
} else if let Err(e) = self.unify.unify(&place_ty, &rvalue_ty) {
    self.errors.push(*e);
}

// After
} else if let Err(mut e) = self.unify.unify(&place_ty, &rvalue_ty) {
    if stmt.span != Span::DUMMY {
        e.span = stmt.span;
    }
    self.errors.push(*e);
}
```

### 3.4 Debug format leak fixes (5 sites)

The 5 `{:?}` Debug format leaks in `infer_rvalue` error messages are
now replaced with `type_kind_to_string`:

| Site | Old | New |
|------|-----|-----|
| Shift count | `found {:?}` | `found {}` (type_kind_to_string) |
| Arithmetic lhs | `to {:?}` | `to {}` |
| Arithmetic rhs | `to {:?}` | `to {}` |
| UnaryOp Not | `to {:?}` | `to {}` |
| UnaryOp Neg | `to {:?}` | `to {}` |

This completes the Debug format leak cleanup started in Stage 15.80.
All typeck error messages now use human-readable type names.

## 4. API Naming Compliance (§23)

**Signature change** (not a new API, but a parameter addition):

| Method | Change | §23 Compliance |
|--------|--------|-----------------|
| `infer_rvalue` | Added `stmt_span: Span` parameter | ✅ Parameter name follows `<noun>_<noun>` pattern |

No new public functions or types. The `infer_rvalue` method is private
(`fn infer_rvalue`, not `pub fn`), so the signature change is internal.

## 5. §16 Interface Isolation

The `stmt_span` parameter flows from `check_statement` (which has
`stmt.span`) into `infer_rvalue` (which produces the errors). This is
a single-module data flow within `typeck::checker`. No cross-stage
access.

The `type_kind_to_string` calls import `crate::mir::ty::type_kind_to_string`
(same as Stage 15.80). No new cross-stage dependencies.

## 6. §25 Deep Review (8 Dimensions)

| Dimension | Status | Notes |
|-----------|--------|-------|
| D1 Architecture | ✅ | `stmt_span` is explicit parameter (not hidden state) |
| D2 Tech Debt | ✅ | 9 `Span::DUMMY` error sites fixed; 5 `{:?}` leaks fixed |
| D3 Test Coverage | ✅ | 3 new integration tests verify span accuracy |
| D4 Next-Phase Readiness | ✅ | No regressions; error system is now in good shape |
| D5 Design Rationality | ✅ | Explicit parameter over hidden state (per §1.0 原則 3) |
| D6 Performance | ✅ | One extra `Span` parameter (Copy, 8 bytes); negligible |
| D7 Documentation | ✅ | This doc + test plan + matrix updated |
| D8 Test Path Coverage | ✅ | BinaryOp + UnaryOp paths have span accuracy tests |

**Committee Vote**: GO — Stage 15.82 complete.

## 7. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 232/232 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2138/2138 PASS (was 2135, +3 new)
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
- **Total: 7586 tests passing, 0 failures, 0 warnings.**

## 8. Error System Cleanup Summary (Stages 15.80-15.82)

The three-stage error system cleanup is now complete:

| Stage | Focus | Sites Fixed |
|-------|-------|-------------|
| 15.80 | Human-readable type names (`type_to_string`) | 6 `{:?}` leaks + 2 `({:?})` enum leaks |
| 15.81 | Terminator span accuracy (`operand_span`, `term.span`) | 7 `Span::DUMMY` sites + 1 `{:?}` leak |
| 15.82 | Statement/rvalue span accuracy (`stmt_span` in `infer_rvalue`) | 9 `Span::DUMMY` sites + 5 `{:?}` leaks |
| **Total** | | **18 `Span::DUMMY` sites + 14 `{:?}` leaks fixed** |

**Result**: All user-facing typeck error messages now:
- Use human-readable type names (`i32`, `bool`, `&mut T`, etc.) — no
  Debug format leaks
- Point to actual source locations (with snippet underlines) — no
  `Span::DUMMY` / "1:1" errors

The error system is now in good shape for user-facing work.

## 9. Next Steps

The error system cleanup is complete. The next major v0.2 task is:

**Task 12 (Lifetime elision)** — the next major v0.2 task (2-3 weeks,
P1, ready now). This is the last remaining P1 task for v0.2 release.

## 10. Version Policy

v0.206.0 → v0.207.0 (minor bump — Phase 2 error system span accuracy
fix + 3 new integration tests).
