# Stage 15.10 — SubstsRef Rc<[Ty]> Interning

> **Author**: redskaber
> **Date**: 2026-07-31
> **Version**: v0.135.0 → v0.136.0
> **Process**: stage-committee-process.md v3.23 §29 (stage-end deep review)
> **v0.2 Phase 1 Task 2 (stepping stone)**: SubstsRef Vec<Ty> → Rc<[Ty]>

## 1. Executive Summary

Stage 15.10 changes `SubstsRef` from `Vec<Ty>` to `Rc<[Ty]>` — an interned
slice that makes `SubstsRef::clone()` a refcount bump instead of a heap
allocation. This is the stepping stone toward full Ty interning (Task 1)
and directly addresses Phase 1 Task 2 ("SubstsRef → `&'tcx [GenericArg]`").

For a crate with 50 generic applications, this eliminates 50 `Vec<Ty>`
heap allocations per compilation. The `Rc<[Ty]>` form also enables future
sharing — when the same `Vec<i32>` type appears in 100 places, they can
all share one `Rc<[i32]>` slice after interning.

## 2. Why This Change?

Per `docs/lang-design/19-ty-interning.md` and Phase 2 audit:
- `SubstsRef = Vec<Ty>` causes per-generic-app heap allocation
- Every `TyKind::Adt(def_id, substs)` construction allocates a new Vec
- Every `.clone()` of a Ty with substs clones the Vec (24 bytes + N × Ty)
- rustc uses `&'tcx [GenericArg<'tcx>]` — interned slice, 0 allocations

The `Rc<[Ty]>` form is the v0.2 stepping stone:
- `Rc::clone()` is a refcount bump (8 bytes, no heap allocation)
- `Rc<[Ty]>` derefs to `[Ty]`, so `.iter()`, `.get()`, `.len()` work unchanged
- Future v0.3 can replace `Rc<[Ty]>` with `&'tcx [Ty]` (arena interning)

## 3. Design

### 3.1 Type change

```rust
// Before (Stage 3.47 - 15.9):
pub type SubstsRef = Vec<Ty>;

// After (Stage 15.10):
pub type SubstsRef = Rc<[Ty]>;
```

### 3.2 Construction patterns

| Pattern | Before | After |
|---------|--------|-------|
| Empty substs | `Vec::new()` | `Vec::<Ty>::new().into()` |
| From Vec | `vec![ty1, ty2]` | `vec![ty1, ty2].into()` |
| Clone existing | `substs.clone()` (Vec clone) | `substs.clone()` (Rc refcount bump) |

### 3.3 Consumption patterns

All consumption patterns work unchanged because `Rc<[Ty]>` derefs to `[Ty]`:
- `substs.iter()` ✅
- `substs.get(i)` ✅
- `substs.len()` ✅
- `substs.is_empty()` ✅
- `for st in substs` → `for st in substs.iter()` (explicit `.iter()` needed)
- `substs[i]` → `substs[i]` (works via Deref, but read-only)

### 3.4 Mutation patterns

`Rc<[Ty]>` is immutable. The one mutation site (writeback closure substs)
was refactored:

```rust
// Before: mutate in place
if let Rvalue::Aggregate(AggregateKind::Closure(_, substs), _) = rv {
    for (i, resolved_ty_opt) in resolved_substs.iter().enumerate() {
        if let Some(ty) = resolved_ty_opt {
            if i < substs.len() {
                substs[i] = ty.clone();  // ← won't work on Rc<[Ty]>
            }
        }
    }
}

// After: rebuild Vec, mutate, convert back
if let Rvalue::Aggregate(AggregateKind::Closure(_, substs), _) = rv {
    let mut new_substs_vec: Vec<Ty> = substs.iter().cloned().collect();
    for (i, resolved_ty_opt) in resolved_substs.iter().enumerate() {
        if let Some(ty) = resolved_ty_opt {
            if i < new_substs_vec.len() {
                new_substs_vec[i] = ty.clone();
            }
        }
    }
    *substs = new_substs_vec.into();  // Vec<Ty> → Rc<[Ty]>
}
```

This preserves the exact old behavior (only updates `Some` entries) while
working with the immutable `Rc<[Ty]>`.

## 4. §29 Stage-End Deep Review

### 4.1 Data flow coverage (§29.1.1)

Data flow is unchanged — `SubstsRef` is still a sequence of `Ty` values.
The only difference is ownership: `Vec<Ty>` (owned) → `Rc<[Ty]>` (shared).
Consumption patterns (`.iter()`, `.get()`, `.len()`) are identical.

### 4.2 Architecture review (§29.1.2)

**Structural clarity** ✅ — `SubstsRef` is now a shared type, reflecting
that substs are often shared across multiple type applications.

**Efficiency** ✅ — eliminates per-generic-app heap allocation. `Rc::clone()`
is O(1) (refcount bump) vs `Vec::clone()` which is O(N) (clones every Ty).

**Extensibility** ✅ — future v0.3 can replace `Rc<[Ty]>` with `&'tcx [Ty]`
(arena interning) with no API change — both deref to `[Ty]`.

### 4.3 Design-impl-test coverage (§29.1.3)

| Design point | Implementation | Test |
|--------------|----------------|------|
| Empty substs construction | `Vec::<Ty>::new().into()` | `stage15_10_struct_empty_substs` |
| Closure substs construction | `capture_tys.into()` | `stage15_10_closure_with_captures` |
| Writeback mutation (rebuild pattern) | `substs.iter().cloned().collect()` + `.into()` | `stage15_10_method_call_on_adt` |
| Nested Adt substs | propagates through writeback | `stage15_10_nested_struct_access` |
| Closure capturing struct | Rc<[Ty]> with Adt element | `stage15_10_closure_capturing_struct` |

### 4.4 Hidden problems (§29.1.4)

| Hidden problem | Complexity growth | Stage 15.10 status |
|----------------|-------------------|-------------------|
| `Rc<[Ty]>` doesn't support `Rc::make_mut` (slice is unsized) | 1× | Acceptable — rebuild pattern used |
| Writeback rebuild allocates a new Vec + Rc | 1× (one alloc per closure writeback) | Acceptable — closures are rare |
| No deduplication yet (same Vec<i32> gets separate Rc) | 2× (v0.3 arena interning) | Deferred to v0.3 |

No new hidden problems. The Rc<[Ty]> form is the standard Rust pattern for
shared immutable slices.

### 4.5 Refactoring optimality (§29.2)

**Approach taken** ✅ — `Rc<[Ty]>` is the standard Rust stepping stone toward
arena interning. It's the same pattern rustc used before moving to `&'tcx`.

**Alternative considered** ✅ — Could have used `Rc<Vec<Ty>>` (supports
`make_mut`). Rejected because it adds an extra indirection (Rc → Vec → data)
and `Rc<[Ty]>` is the target form for v0.3 arena interning.

**Skipped refactors** ✅ — Did not change `Box<Ty>` to `Ty` in TyKind variants
(Ref, RawPtr, Array, Slice). That's the full Ty interning (Task 1), which is
a larger effort. Per §15 "最优 > 最小": one type change per stage.

## 5. Test Results

| Test suite | Before (v0.135.0) | After (v0.136.0) | Delta |
|------------|-------------------|------------------|-------|
| Rust unit (lib) | 145 | 145 | 0 |
| Rust integration (all_tests) | 1976 | 1983 | +7 (SubstsRef tests) |
| Conformance (.lin) | 5216 | 5216 | 0 |
| **Total** | **7337** | **7344** | **+7** |

All tests pass. Zero regressions. Zero clippy warnings. fmt clean.

## 6. v0.2 Phase 1 Progress Update

| Task | Status | Notes |
|------|--------|-------|
| 1. Ty interning (`Ty<'tcx>` Copy) | Design done (Stage 15.1) | Implementation deferred to v0.3 |
| **2. SubstsRef → `&'tcx [GenericArg]`** | **Stepping stone done (Stage 15.10)** | Rc<[Ty]> is the v0.2 form; v0.3 arena interning |
| 3. TraitResolver key redesign | Not started | Blocked on Tasks 1+2 |
| 4. EmitValue → typed LLVM handle | Not started | Independent |
| 5. Consolidate 8 writeback passes → 2 | ✅ Done (Stage 15.7) | 650 LOC → 25 LOC |
| Quick win: AdtLayouts crate-level | ✅ Done (Stage 15.8) | ~500KB memory saved |
| Quick win: VtableEntry.fn_name interning | ✅ Done (Stage 15.9) | HP-B16 closed |
| Quick win: Stop stringifying CoherenceError | ✅ Done (Stage 15.9) | TraitError enum added |

Stage 15.10 completes the v0.2 stepping stone for Task 2. The remaining
major work is Task 1 (full Ty interning via Rc<TyKind>) and Task 3
(TraitResolver key redesign).

## 7. Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Version bump 0.135.0 → 0.136.0 |
| `src/mir/ty.rs` | `SubstsRef: Vec<Ty>` → `Rc<[Ty]>`; added `use std::rc::Rc;` |
| `src/mir/lower/expr_operand.rs` | 12 `Vec::new()` → `Vec::<Ty>::new().into()`; 3 `vec![]` → `vec![].into()`; `substs.clone()` → `substs.iter().cloned().collect()`; `capture_tys.clone()` → `capture_tys.clone().into()` |
| `src/mir/lower/control_flow.rs` | 1 `Vec::new()` → `Vec::<Ty>::new().into()` |
| `src/mir/lower/mod.rs` | 1 `Vec::new()` → `Vec::<Ty>::new().into()` |
| `src/mir/lower/adt_layout.rs` | 2 `for st in substs` → `for st in substs.iter()` |
| `src/mir/lower/writeback.rs` | Mutation site: rebuild Vec + `.into()` pattern; `new_substs.clone()` → `new_substs.clone().into()` |
| `src/codegen/rvalue.rs` | 1 `Vec::new()` → `Vec::<Ty>::new().into()` |
| `src/borrowck/mod.rs` | 1 `vec![]` → `vec![].into()` (test) |
| `tests/v0/stage5/plan/copy_unification_tests.rs` | 3 `vec![]` → `vec![].into()` |
| `tests/v0/stage5/plan/ty_is_copy_tests.rs` | 1 `vec![]` → `vec![].into()` |
| `tests/v0/stage15/plan/substs_ref_rc_tests.rs` | **NEW** — 7 integration tests |
| `tests/all_tests.rs` | Registered `stage15_substs_ref_rc_tests` |
| `docs/develop/v0/stage-15/stage-15.10-substs-ref-rc.md` | This document |
| `docs/tests/v0/stage15/stage-15.10-test-plan.md` | **NEW** — test plan |
| `docs/worklog.md` | Stage 15.10 entry appended |
| `RELEASE_NOTES.md` | v0.136.0 entry appended |
| `README.md` | Updated with Stage 15.10 progress |
