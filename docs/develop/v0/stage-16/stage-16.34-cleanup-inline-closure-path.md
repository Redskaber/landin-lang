# Stage 16.34 — Task 10 Step 5: Clean Up Deprecated Inline Closure Path

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.230.3 → v0.231.0
> **Process**: stage-committee-process.md v3.24 §1.0 原則 5 "去除兼容思维" + §23 rule 5 (DRY)

## 1. Executive Summary

Stage 16.34 completes **Task 10 Step 5** — the final cleanup of the closure
redesign. All deprecated inline closure path code is removed, and the
`closure_bodies` side-table (TD-CLOSURE-2) is eliminated.

**What was removed**:
1. `lower_closure_call_inline` function (deprecated since Stage 16.29)
2. `ClosureBodyInfo` struct (only used by inline path)
3. `closure_bodies` field on `MirLowerCtxt` (side-table for inline path)
4. `closure_bodies` insertion in closure literal lowering
5. `closure_bodies` propagation in let binding lowering

**What replaced it**:
- Type-based check: `TyKind::Closure(_, _)` on the func local's type
- The `SynthesizedClosureFunction` metadata is the single source of truth

**Test results**: 7780 tests passing (244 lib + 2312 integration + 5224
conformance), 0 failures, 0 warnings. No behavior change — all closure
patterns still work identically.

## 2. Root Cause: Why the Side-Table Existed

The `closure_bodies` side-table was introduced in Stage 13.3a (TD-030) as
part of the inline closure call approach. It mapped `LocalId → ClosureBodyInfo`
so the call site could look up the closure's (params, body, captures) by
the func local's ID.

The inline approach was a pragmatic subset of Strategy A — it worked for
the common case (`let f = |x| ...; f(5);`) but had limitations:
- Code bloat (closure body duplicated at each call site)
- No optimization (LLVM can't deduplicate MIR-level inlining)
- MIR pollution (call sites polluted with closure body statements)

Stage 16.29 switched to the synthesized `call` function path (Strategy A),
making the inline path (and its `closure_bodies` side-table) obsolete.

## 3. The 通解 Fix: Type-Based Check

**Before** (side-table lookup):
```rust
if cx.closure_bodies.contains_key(&func_local) {
    return lower_closure_call_to_synthesized(cx, func_local, &arg_locals, expr);
}
```

**After** (type-based check):
```rust
let is_closure_typed = cx.mir.local_decls.get(func_local.0 as usize)
    .map(|ld| matches!(&ld.ty.kind, TyKind::Closure(_, _)))
    .unwrap_or(false);
if is_closure_typed {
    return lower_closure_call_to_synthesized(cx, func_local, &arg_locals, expr);
}
```

**Why this works**: The closure literal's local has type `Closure(def_id, substs)`
(concrete, not Infer) at MIR lowering time. Let-bound closures (`let g = |x| ...;`)
inherit this type via the let lowering (control_flow.rs line 598-604: uses
init_local's type if not Infer). So the type system is the single source of
truth for "is this local a closure?".

Per §1.0 原則 5 "去除兼容思维": dead side-table removed.
Per §1.0 原則 6 "通用 > 特例": one type-based check for all closure-typed locals.
Per §23 rule 5 (DRY): type + SynthesizedClosureFunction is the single source of truth.

## 4. Files Changed

### 4.1 src/mir/lower/expr_operand.rs
- Removed `lower_closure_call_inline` function (100 lines)
- Removed `closure_bodies` insertion in closure literal lowering
- Replaced `closure_bodies.contains_key` with type-based check

### 4.2 src/mir/lower/mod.rs
- Removed `ClosureBodyInfo` struct
- Removed `closure_bodies` field from `MirLowerCtxt`
- Removed `closure_bodies` initialization from `new` and `new_with_unify`

### 4.3 src/mir/lower/control_flow.rs
- Removed `closure_bodies` propagation in let binding lowering

## 5. Technical Debt Update

| ID | Description | Priority | Status |
|----|-------------|----------|--------|
| TD-CLOSURE-2 | `closure_bodies` side-table duplicates `synthesized_closure_functions` | P3 | ✅ **FIXED** (Stage 16.34) |
| TD-COPY-1 | `ty_is_copy` deprecated (test-only) | P3 | ✅ Documented |
| TD-FALLBACK-1 | `BorrowChecker::new()` unsound (test-only) | P3 | ✅ Documented |

**All closure TDs are now CLOSED.** The closure redesign is 100% complete
with no remaining technical debt.

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2312/2312 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7780 tests passing, 0 failures, 0 warnings.**
- **Runtime**: f(10)=11 ✅, f()()()=42 ✅, mut_cap=3 ✅

## 7. Version Policy

v0.230.3 → v0.231.0 (minor bump — dead code removal, API surface change:
`ClosureBodyInfo` struct and `closure_bodies` field removed. No behavior
change, but downstream code referencing these would break.)

## 8. v0.3 Closure Redesign — Final Status

Task 10 is now **100% COMPLETE** with all 5 steps done:
- Step 1: SynthesizedClosureFunction infrastructure ✅ (Stage 16.13)
- Step 2: build_synthesized_closure_mir_body ✅ (Stage 16.14)
- Step 3: lower_closure_call_to_synthesized ✅ (Stage 16.16-16.32)
- Step 4: codegen_synthesized_closure_functions ✅ (Stage 16.16-16.32)
- Step 5: Clean up deprecated inline path ✅ (Stage 16.34)

**No deprecated closure APIs remain.** The closure redesign has a clean
API surface with no dead code.

## 9. References

- Task 10 design: `docs/develop/v0/task-10-closure-redesign-design.md`
- v0.3 design: `docs/develop/v0/v0.3-complete-design.md`
- Stage 16.29 (typeck gap fix): `docs/develop/v0/stage-16/stage-16.29-typeck-on-synthesized-closure-mir.md`
- Stage committee process: `docs/stage-committee-process.md` §1.0 原則 5, §23
