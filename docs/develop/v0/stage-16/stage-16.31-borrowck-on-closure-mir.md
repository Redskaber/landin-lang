# Stage 16.31 — 通解: Borrowck on Closure MIR Bodies (Capture Mutability Propagation)

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.230.1 → v0.230.2
> **Process**: stage-committee-process.md v3.24 §1.0 原則 4 "报错 > 静默" + 原則 9 "正确 > 妥协"

## 1. Executive Summary

Stage 16.31 fixes **TD-CLOSURE-BORROWCK-1** — the soundness gap where
borrowck was silently skipped on closure MIR bodies. The fix propagates
capture mutability from the outer scope to the extract locals in the
closure MIR body, then enables borrowck on closure MIR bodies.

**Root cause**: When `build_synthesized_closure_mir_body` extracted
captures from `self`, it created the extract locals as `Immutable` (via
`new_local`). When the closure body mutated a captured `mut` variable
(e.g., `|| { while x<3 { x+=1; } x }`), borrowck flagged `x+=1` as
"cannot assign twice to immutable variable".

Stage 16.29 worked around this by SKIPPING borrowck on closure MIR
bodies — a soundness gap (violates §1.0 原則 4 "报错 > 静默").

**The 通解 fix**:
1. Track capture mutability in `SynthesizedClosureFunction.captures`
   (4-tuple: `(HirId, field_idx, Ty, Mutability)`)
2. In `build_synthesized_closure_mir_body`, create extract locals with
   the captured variable's mutability (via `new_local_with_mut`)
3. Also make the return local `Mutable` (matching main body's G5 fix)
4. Enable borrowck on closure MIR bodies in the driver

**Test results**: 7744 tests passing (244 lib + 2276 integration + 5224
conformance), 0 failures, 0 warnings.

**Runtime verification**:
- `f() = 3` ✅ (`|| { while x<3 { x+=1; } x }` with captured `mut x`)
- Early return closures work (`|| { if x>0 { return 1; } 0 }`)

## 2. Root Cause Analysis

### 2.1 The Soundness Gap

Stage 16.29 enabled typeck on closure MIR bodies (the 通解 for the typeck
gap), but SKIPPED borrowck on closure MIR bodies because of false
positives:

```rust
// Stage 16.29 driver code (SKIPPED borrowck):
// TD-CLOSURE-BORROWCK-1: Borrowck on closure MIR bodies (P2, follow-up).
```

This is a soundness gap — borrow violations inside closure bodies
(e.g., use-after-move, double-mut-borrow) were silently ignored.

### 2.2 The False Positive Root Cause

When `build_synthesized_closure_mir_body` extracts captures:

```rust
// Before Stage 16.31:
for (cap_hir_id, field_idx, cap_ty) in &func.captures {
    let extract_local = cx.mir.new_local(cap_ty.clone(), None, func.body.span);
    // ^^^ new_local = Immutable (default)
    ...
}
```

The extract local is `Immutable`. When the closure body does `x += 1`
(where `x` is a captured `mut` variable), the assignment goes to
`extract_local`. Borrowck's `check_place_write` sees:
- `extract_local` is `Immutable`
- `extract_local` was initialized (by the capture extract assignment)
- Second write (`x += 1`) → "cannot assign twice to immutable variable"

### 2.3 The Fix (通解)

**Step 1**: Track capture mutability in `SynthesizedClosureFunction.captures`:

```rust
// Before: (HirId, u32, Ty)
// After:  (HirId, u32, Ty, Mutability)
pub captures: Vec<(HirId, u32, Ty, Mutability)>,
```

The mutability is read from the captured variable's `local_decl.mutability`
during capture collection in `expr_operand.rs`.

**Step 2**: Use `new_local_with_mut` in `build_synthesized_closure_mir_body`:

```rust
for (cap_hir_id, field_idx, cap_ty, cap_mutability) in &func.captures {
    let extract_local = cx.mir.new_local_with_mut(
        cap_ty.clone(), None, func.body.span, *cap_mutability
    );
    ...
}
```

**Step 3**: Make the return local `Mutable` (matching main body's G5 fix):

```rust
let return_local = cx.mir.new_local_with_mut(
    return_ty, None, func.body.span, Mutability::Mutable,
);
```

This allows `return expr;` inside closure bodies to assign to `LocalId(0)`
without borrowck flagging "cannot assign twice to immutable variable"
(the first assign is the body result, the second is the early return).

**Step 4**: Enable borrowck on closure MIR bodies in the driver:

```rust
let mut closure_bc = borrowck::BorrowChecker::with_resolver_and_sigs(...);
closure_bc.check_mir_body_with_dataflow(&closure_mir);
errors.borrowck.extend(closure_bc.into_errors());
```

## 3. Architecture Changes

### 3.1 SynthesizedClosureFunction.captures (4-tuple)

**Before**: `Vec<(HirId, u32, Ty)>` — 3-tuple, no mutability info.

**After**: `Vec<(HirId, u32, Ty, Mutability)>` — 4-tuple, includes the
captured variable's mutability from the outer scope.

### 3.2 build_synthesized_closure_mir_body

- Extract locals use `new_local_with_mut` with the captured mutability
- Return local is `Mutable` (was `Immutable`)

### 3.3 Driver

- Borrowck on closure MIR bodies is ENABLED (was skipped in Stage 16.29)
- Borrow violations inside closure bodies are now reported

## 4. Test Coverage

### 4.1 Compile Coverage

- ✅ All 7744 tests pass (no regressions)
- ✅ `|| { while x<3 { x+=1; } x }` compiles (was false positive before)
- ✅ `|| { if x>0 { return 1; } 0 }` compiles (was false positive before)

### 4.2 Runtime Coverage

| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| `f()` where `f = \|\| { while x<3 { x+=1; } x }` | 3 | 3 | ✅ **NEW** |
| `f()` where `f = \|\| { if x>0 { return 1; } 0 }` | 1 | 1 | ✅ |
| `f(10)` (no-capture) | 11 | 11 | ✅ |
| `x + y` (i32 capture) | 15 | 15 | ✅ |
| `f()()` (nested) | 42 | 42 | ✅ |

## 5. Technical Debt Update

| ID | Description | Priority | Status |
|----|-------------|----------|--------|
| TD-CLOSURE-BORROWCK-1 | Borrowck on closure MIR bodies | P2 | ✅ **FIXED** (Stage 16.31) |
| TD-CLOSURE-TRIPLE-1 | Triple-nested closure typeck | P3 | 🔧 Follow-up |
| TD-CLOSURE-2 | `closure_bodies` side-table duplicates `synthesized_closure_functions` | P3 | 🔧 Step 5 cleanup |
| TD-COPY-1 | `ty_is_copy` deprecated (test-only) | P3 | ✅ Documented |

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2276/2276 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7744 tests passing, 0 failures, 0 warnings.**
- **Runtime**: `f()=3` ✅ **NEW** (mutable capture loop), early return ✅

## 7. Version Policy

v0.230.1 → v0.230.2 (patch bump — capture mutability propagation +
borrowck on closure MIR bodies enabled. No API changes for existing
code; `SynthesizedClosureFunction.captures` extended to 4-tuple.)

## 8. References

- Stage 16.29 (typeck gap fix): `docs/develop/v0/stage-16/stage-16.29-typeck-on-synthesized-closure-mir.md`
- Stage 16.30 (codegen fix): `docs/develop/v0/stage-16/stage-16.30-closure-typed-call-codegen.md`
- Task 10 design: `docs/develop/v0/task-10-closure-redesign-design.md`
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md`
- Stage committee process: `docs/stage-committee-process.md` §1.0 原則 4, 9
