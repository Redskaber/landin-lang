# Stage 15.7 — Writeback Consolidation (8 passes → 2 functions)

> **Author**: redskaber
> **Date**: 2026-07-31
> **Version**: v0.132.0 → v0.133.0
> **Process**: stage-committee-process.md v3.23 §29 (stage-end deep review)
> **v0.2 Phase 1 Task 5**: Consolidate 8 driver writeback passes → 2

## 1. Executive Summary

Stage 15.7 consolidates the 8 incremental driver writeback passes (Stages
14.30-14.84) into 2 functions in a new `src/mir/lower/writeback.rs` module:

- `writeback_type_propagation(mir, fn_sigs)` — merges passes 1-5 (Tuple
  Aggregate, Call dest, Field projection Copy, Index projection Copy,
  Copy/Move chain fixpoint) into a single fixpoint walk.
- `writeback_closures(mir)` — merges passes 6-8 (Closure substs, Closure
  local_decl.ty, Closure extract locals) into a single 3-sub-pass walk.

The driver's writeback section shrinks from ~650 LOC of inline code to
~25 LOC of function calls. The driver is now truly orchestrator-only
(per §16) — it calls the writeback functions in order, the functions
contain the logic.

A fixpoint convergence bug was found and fixed during integration testing:
the fixpoint loop must not push `Infer`/`Error` types to `changes` (doing
so causes an infinite loop when, e.g., a generic method's return type `T`
is `Param` and gets lowered to `Infer`). The fix adds a `!needs_writeback(&new_ty)`
guard before pushing.

## 2. Why Consolidate?

Per `docs/develop/v0/stage-15/v0.2-preparation.md` Phase 1 Task 5:
"Consolidate 8 writeback passes → 2. 1-2 weeks, needs own stage."

The 8 incremental passes were correct for v0.1 (each fixed a real bug),
but they had three problems:

1. **Performance**: 8 separate O(B×S) walks per body (B = basic blocks,
   S = statements). For a 100-block body, that's 800 block-walks vs the
   consolidated 100 (one fixpoint walk that converges in ~3 iterations).

2. **Maintainability**: The 8 passes were inline in `driver.rs`, making
   the driver 2,358 LOC total — half of which was writeback logic. Per
   §16, the driver should be orchestrator-only.

3. **Testability**: The inline passes couldn't be unit-tested in isolation.
   Each could only be tested via end-to-end `compile()` calls, which made
   regressions hard to localize.

## 3. Design

### 3.1 Module structure

```
src/mir/lower/
├── writeback.rs   ← NEW (Stage 15.7)
├── mod.rs         ← re-exports writeback_type_propagation, writeback_closures
├── expr_operand.rs
├── ...
```

Per §23 (API Naming): both functions follow the `<verb>_<noun>` pattern.
Per §16 (interface isolation): pure MIR-to-MIR transforms, no HIR access.

### 3.2 `writeback_type_propagation` — consolidated passes 1-5

**Signature**:
```rust
pub fn writeback_type_propagation(
    mir: &mut MirBody,
    fn_sigs: &HashMap<DefId, Sig>,
)
```

**Algorithm**: fixpoint loop. Each iteration walks all basic blocks once,
applying all 5 rules. The loop exits when an iteration makes no changes.

**Rules applied per iteration**:

| # | Rule | Trigger | Action |
|---|------|---------|--------|
| 1 | Tuple Aggregate | `loc = (a, b, c)` | Build `Tuple([a_ty, b_ty, c_ty])` from operand types |
| 2 | Call dest | `loc = call f(...)` | Look up `f`'s return type in `fn_sigs` |
| 3 | Field projection | `loc = Copy(tup.0)` | Resolve field type from `tup`'s Tuple type |
| 4 | Index projection | `loc = Copy(arr[i])` | Resolve element type from `arr`'s Array type |
| 5 | Local-to-local Copy/Move | `loc = Copy(other)` | Propagate `other`'s resolved type |

**Convergence guard** (Stage 15.7 fix): each rule only pushes to `changes`
if the new type is NOT `Infer`/`Error`. Without this guard, Rule 2 would
push `Infer` (from a generic method's `Param` return type) to dest, dest
stays `Infer`, and the loop never terminates.

**Termination proof**: each iteration can only transition a local from
`Infer`/`Error` to a concrete type (never the reverse). The number of
locals is finite, so the loop runs at most `local_count + 1` iterations.

### 3.3 `writeback_closures` — consolidated passes 6-8

**Signature**:
```rust
pub fn writeback_closures(mir: &mut MirBody)
```

**Algorithm**: 3 linear sub-passes (no fixpoint needed — the dependency
chain is linear).

**Sub-passes**:

1. **Closure substs + local_decl.ty**: For each
   `loc = Aggregate(Closure(def_id, _), operands)`:
   - Resolve each subst from the corresponding operand's source local type.
   - Update the Aggregate's substs in place.
   - Update `loc`'s local_decl.ty to `Closure(def_id, resolved_substs)`.

2. **Closure Move propagation**: For each `loc = Use(Move(closure_tmp))`:
   - If `closure_tmp`'s type is now `Closure(_, [resolved])` (no Infer),
     propagate it to `loc`'s local_decl.ty.

3. **Closure extract locals**: For each
   `loc = Use(Copy(Projection(closure_local, Field(i, _))))`:
   - Look up `closure_local`'s resolved subst at index `i`.
   - Write it to `loc`'s local_decl.ty.

**Why no fixpoint?**: Sub-pass 1 produces resolved substs. Sub-pass 2
consumes them to propagate to user-visible locals. Sub-pass 3 consumes
sub-pass 2's result to update extract locals. The chain is linear — each
sub-pass runs exactly once, in order.

### 3.4 Driver integration

The driver's per-body writeback section (previously ~650 LOC) is now:

```rust
// Stage 15.7 (v0.2 writeback consolidation)
crate::mir::lower::writeback_type_propagation(&mut mir, &fn_sig_table.sigs);

// Re-populate adt_layouts after type propagation writeback
crate::mir::lower::populate_adt_layouts(&mut mir, &hir);

crate::mir::lower::writeback_closures(&mut mir);

// Also re-populate adt_layouts after closure subst writeback
crate::mir::lower::populate_adt_layouts(&mut mir, &hir);
```

The two `populate_adt_layouts` calls are preserved because the writeback
may expose new Adt DefIds that weren't in local_decls before.

## 4. The Infinite-Loop Bug (and Fix)

### 4.1 What happened

After the initial consolidation, 5 conformance tests hung with TIMEOUT:
- `01-typecheck/02-generics/006-generic-method.lin`
- `02-borrowck/99-error-cases/bk-0464-aa4-repeat-unresolved.lin`
- `02-borrowck/99-error-cases/bk-0470-me3-repeat-nonliteral.lin`
- `04-e2e/06-run-ok/e2e-runok-026-array-repeat.lin`
- `06-stdlib/02-std/015-clone-impl.lin`

### 4.2 Root cause

The fixpoint loop in `writeback_type_propagation` pushed a change whenever
a rule produced a new type — but didn't check if the new type was itself
`Infer`/`Error`. 

For the generic method test (`fn f<T>(&self, x: T) -> T`), the return
type `T` is `TyKind::Param` in HIR but gets lowered to `TyKind::Infer`
in MIR (v0.1 doesn't support generics). So:

1. Iteration 1: Rule 2 (Call dest) reads `sig.output` → `Infer`. Pushes
   `(dest_idx, Infer)` to `changes`. After apply, `dest.ty = Infer`.
2. Iteration 2: `dest.ty` is still `Infer` → `needs_writeback` returns
   true → Rule 2 fires again → pushes `(dest_idx, Infer)` again.
3. Infinite loop.

### 4.3 Fix

Added a convergence guard before pushing to `changes`:

```rust
if let Some(new_ty) = compute_writeback_ty(rvalue, mir) {
    if !needs_writeback(&new_ty) {  // ← Stage 15.7 fix
        changes.push((dest_idx, new_ty));
    }
}
```

This matches the behavior of the OLD Pass 5 (Copy/Move chain), which
only fired when `src_ty` was NOT `Infer`/`Error`. The consolidated loop
now applies the same guard to all 5 rules uniformly.

### 4.4 Regression test

`stage15_7_generic_method_no_hang_regression` in
`tests/v0/stage15/plan/writeback_consolidation_tests.rs` verifies the
fix: a generic method call must produce a compile error (v0.1 limitation),
NOT hang.

Per §1.0 原则 5 "报错 > 静默": errors are better than silent hangs.

## 5. §29 Stage-End Deep Review

### 5.1 Data flow coverage (§29.1.1)

The consolidation preserves the exact data flow of the 8 original passes:
- MIR local_decls are read (source types) and written (dest types).
- `fn_sigs` is read-only (Call dest rule).
- No HIR access (per §16).

No new catch-all branches. The `compute_writeback_ty` helper returns
`None` for unsupported rvalue kinds (BinaryOp, UnaryOp, Ref, Cast,
Array/Adt Aggregate) — these were also unhandled by the original passes.

### 5.2 Architecture review (§29.1.2)

**Structural clarity** ✅ — writeback logic is now in `src/mir/lower/writeback.rs`,
not inline in the driver. The driver is orchestrator-only (per §16).

**Efficiency** ✅ — 8 O(B×S) walks → 1 fixpoint walk (~3 iterations) +
1 closure walk (3 sub-passes). Net: ~6× fewer block-walks.

**Extensibility** ✅ — adding a new writeback rule means adding a branch
to `compute_writeback_ty`, not adding a 9th inline pass to the driver.

### 5.3 Design-impl-test coverage (§29.1.3)

| Design point | Implementation | Test |
|--------------|----------------|------|
| 8 passes → 2 functions | `writeback.rs` module | Unit tests (5) + integration tests (7) |
| Fixpoint convergence | `loop { ... if changes.is_empty() { break; } }` | `stage15_7_fixpoint_copy_chain` |
| Convergence guard (no Infer push) | `if !needs_writeback(&new_ty)` | `stage15_7_generic_method_no_hang_regression` |
| Closure 3-sub-pass linearity | Sub-passes 1→2→3 in sequence | `stage15_7_closure_writeback_integration` |

### 5.4 Hidden problems (§29.1.4)

| Hidden problem | Complexity growth | Stage 15.7 status |
|----------------|-------------------|-------------------|
| Tuple Aggregate writes Tuple([Infer, Infer]) if operands unresolved | 1× (same as v0.1) | Preserved (not a regression) |
| Closure writeback is 3 sub-passes, not 1 | 1× (linear, no fixpoint) | Acceptable for v0.2 |
| `populate_adt_layouts` called 2× per body | 1× (cached after first call) | v0.3: share crate-level |

No new hidden problems introduced. The convergence guard actually FIXES
a latent issue (the OLD Pass 5 fixpoint would also infinite-loop if src
was Infer — but it never was, because Pass 5 only matched `Local(src_id)`
and checked `!needs_writeback(src_ty)`).

### 5.5 Refactoring optimality (§29.2)

**Approach taken** ✅ — Pure extraction + consolidation. No semantic
changes (except the convergence guard, which is a bug fix).

**Alternative considered** ✅ — Could have merged closure writeback into
the main fixpoint loop. Rejected because closure writeback is linear
(sub-pass 1→2→3), not fixpoint — merging would add unnecessary iterations.

**Skipped refactors** ✅ — Did not merge the two `populate_adt_layouts`
calls into one. They serve different purposes: the first picks up types
exposed by type propagation, the second picks up types exposed by
closure writeback. Merging would require running both writeback passes
first, then one `populate_adt_layouts` — but that would miss Adt types
that closure writeback needs. Per §15 "最优 > 最小": two calls is correct.

## 6. Test Results

| Test suite | Before (v0.132.0) | After (v0.133.0) | Delta |
|------------|-------------------|------------------|-------|
| Rust unit (lib) | 140 | 145 | +5 (writeback unit tests) |
| Rust integration (all_tests) | 1957 | 1964 | +7 (writeback integration tests) |
| Conformance (.lin) | 5216 | 5216 | 0 |
| **Total** | **7313** | **7325** | **+12** |

All tests pass. Zero regressions. Zero clippy warnings. fmt clean.

## 7. v0.2 Phase 1 Progress Update

| Task | Status | Notes |
|------|--------|-------|
| 1. Ty interning (`Ty<'tcx>` Copy) | Design done (Stage 15.1) | Implementation deferred to v0.3 |
| 2. SubstsRef → `&'tcx [GenericArg]` | Not started | Blocked on Task 1 |
| 3. TraitResolver key redesign | Not started | Blocked on Tasks 1+2 |
| 4. EmitValue → typed LLVM handle | Not started | Independent |
| **5. Consolidate 8 writeback passes → 2** | ✅ **Done (Stage 15.7)** | 650 LOC → 25 LOC in driver |
| Side-quest: Span removal from Ty | ✅ Done (Stage 15.5) | Unblocked Tasks 1, 6, 9 |
| Side-quest: method_return_type_cache | ✅ Done (Stage 15.6) | Closes HP-B12 |
| Side-quest: §23 API naming audit | ✅ Done (Stage 15.6) | Zero violations |

Stage 15.7 closes Phase 1 Task 5. The next stage (15.8) should tackle
Task 1 (Rc<TyKind> stepping-stone Ty interning) — the biggest single
improvement for v0.2, unblocking Tasks 2, 3, and 9.

## 8. Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Version bump 0.132.0 → 0.133.0 |
| `src/mir/lower/writeback.rs` | **NEW** — 2 consolidated writeback functions + 5 unit tests |
| `src/mir/lower/mod.rs` | Registered `writeback` module, re-exported 2 functions |
| `src/driver.rs` | Replaced 650 LOC of inline passes with 25 LOC of function calls (2358 → 1709 LOC total) |
| `tests/all_tests.rs` | Registered `stage15_writeback_consolidation_tests` module |
| `tests/v0/stage15/plan/writeback_consolidation_tests.rs` | **NEW** — 7 integration tests |
| `docs/develop/v0/stage-15/stage-15.7-writeback-consolidation.md` | This document |
| `docs/tests/v0/stage15/stage-15.7-test-plan.md` | **NEW** — test plan |
| `docs/worklog.md` | Stage 15.7 entry appended |
| `RELEASE_NOTES.md` | v0.133.0 entry appended |
| `README.md` | Updated with Stage 15.7 progress |
