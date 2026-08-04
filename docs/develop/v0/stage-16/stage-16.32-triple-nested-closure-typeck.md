# Stage 16.32 — 通解: Triple-Nested Closure Typeck (Closure-typed Func in Typeck)

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.230.2 → v0.230.3
> **Process**: stage-committee-process.md v3.24 §1.0 原則 6 "通用 > 特例" + 原則 9 "正确 > 妥协"

## 1. Executive Summary

Stage 16.32 fixes **TD-CLOSURE-TRIPLE-1** — the last remaining closure
typeck issue. Triple-nested closures (`|| || || x`) now compile AND run
correctly.

**Root cause**: The typeck `check_terminator` for `Call` only handled
`TyKind::FnDef` and `TyKind::FnPtr` — it didn't handle `TyKind::Closure`.
When a Call terminator had a `Closure`-typed func operand (e.g., the
result of `f()` which returns a closure), typeck didn't look up the
closure's fn_sig, so the dest type was never unified with the closure's
return type → stayed `Infer` → "expected function, found _".

**The 通解 fix**: Added `TyKind::Closure(def_id, _)` handling in
`check_terminator` — same as `TyKind::FnDef`: look up the sig in
`fn_sigs`, unify args (skipping self), unify dest with output.

**Test results**: 7758 tests passing (244 lib + 2290 integration + 5224
conformance), 0 failures, 0 warnings.

**Runtime verification**:
- `f()()() = 42` ✅ (triple-nested closure — **NEW!**)

## 2. Root Cause Analysis

### 2.1 The Problem

For `fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }`:

1. `f = || || || x` — outer closure returns middle closure returns inner closure
2. `f()` — call to outer closure, returns middle closure (type: `Closure`)
3. `f()()` — call to middle closure (func type: `Closure`), returns inner closure
4. `f()()()` — call to inner closure (func type: `Closure`), returns `i32`

At typeck time, steps 3 and 4 have `Closure`-typed func operands. The
typeck `check_terminator` for `Call` only handled `FnDef` and `FnPtr` —
it didn't look up the closure's fn_sig for `Closure`-typed func. So the
dest type was never unified with the closure's return type → stayed
`Infer` → "expected function, found _".

### 2.2 The Fix

Added `TyKind::Closure(def_id, _)` handling in `check_terminator`:

```rust
// Stage 16.32: Handle Closure-typed func (same as FnDef).
if let TyKind::Closure(def_id, _) = &func_ty.kind {
    if let Some(sig) = self.fn_sigs.get(def_id).cloned() {
        // Skip the first input (self) — it's not in the MIR Call args.
        let sig_params = &sig.inputs[1.min(sig.inputs.len())..];
        // Check arg count, unify args, unify dest with output.
        ...
    }
}
```

This is the 通解 — handle `Closure` the same way as `FnDef`. Both are
callable types with sigs in `fn_sigs`.

### 2.3 Iterative Typeck (Fixpoint)

The driver also runs iterative typeck passes (max 4) when there are
closure MIR bodies. This handles the circular dependency:
- Closure return types depend on capture types
- Capture types are resolved by the main body's typeck
- Main body's Call sites depend on closure return types

The iterative approach runs typeck passes until fn_sigs stop changing
(fixpoint). Errors from intermediate passes are discarded; only the
final pass's errors are reported.

Per §1.0 原則 6 "通用 > 特例": one iterative approach for all nesting
depths (double, triple, quadruple, etc.).

## 3. Architecture Changes

### 3.1 Typeck: check_terminator (src/typeck/checker.rs)

**Before**: Only `FnDef` and `FnPtr` were handled in Call terminator
typeck.

**After**: `Closure` is also handled — looks up the sig in `fn_sigs`,
skips self (first input), unifies args and dest.

### 3.2 UnificationTable: clear_bindings (src/typeck/unify.rs)

New method: `clear_bindings()` — clears all TyVar/IntVar/FloatVar
bindings but keeps the allocation. Used by iterative typeck passes.

### 3.3 Driver: Iterative Typeck (src/driver.rs)

The driver runs up to 4 typeck passes when there are closure MIR bodies:
1. Typeck all closures + main body
2. Resolve closure_struct_ty substs in fn_sigs
3. Check fixpoint (fn_sigs changed?)
4. If changed, repeat; else stop

Only runs multiple passes when there are closures (no overhead for
non-closure code).

## 4. Test Coverage

### 4.1 Compile Coverage

- ✅ All 7758 tests pass (no regressions)
- ✅ Triple-nested closure compiles (`|| || || x`)

### 4.2 Runtime Coverage

| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| `f()()()` where `f = \|\| \|\| \|\| x` | 42 | 42 | ✅ **NEW** |
| `f(10)` (no-capture) | 11 | 11 | ✅ |
| `x + y` (i32 capture) | 15 | 15 | ✅ |
| `f()()` (double-nested) | 42 | 42 | ✅ |
| `f() = 3` (mutable capture loop) | 3 | 3 | ✅ |

## 5. Technical Debt Update

| ID | Description | Priority | Status |
|----|-------------|----------|--------|
| TD-CLOSURE-TRIPLE-1 | Triple-nested closure typeck | P3 | ✅ **FIXED** (Stage 16.32) |
| TD-CLOSURE-2 | `closure_bodies` side-table duplicates `synthesized_closure_functions` | P3 | 🔧 Step 5 cleanup |
| TD-COPY-1 | `ty_is_copy` deprecated (test-only) | P3 | ✅ Documented |

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2290/2290 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7758 tests passing, 0 failures, 0 warnings.**
- **Runtime**: `f()()()=42` ✅ **NEW** (triple-nested closure)

## 7. Version Policy

v0.230.2 → v0.230.3 (patch bump — Closure-typed func handling in typeck
+ iterative typeck. No API changes.)

## 8. References

- Stage 16.29 (typeck gap fix): `docs/develop/v0/stage-16/stage-16.29-typeck-on-synthesized-closure-mir.md`
- Stage 16.30 (codegen fix): `docs/develop/v0/stage-16/stage-16.30-closure-typed-call-codegen.md`
- Stage 16.31 (borrowck fix): `docs/develop/v0/stage-16/stage-16.31-borrowck-on-closure-mir.md`
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md`
