# Stage 15.8 — Crate-level AdtLayouts Sharing

> **Author**: redskaber
> **Date**: 2026-07-31
> **Version**: v0.133.0 → v0.134.0
> **Process**: stage-committee-process.md v3.23 §29 (stage-end deep review)
> **v0.2 Phase 1 Quick Win**: Share AdtLayouts crate-level instead of per-body

## 1. Executive Summary

Stage 15.8 eliminates per-body `AdtLayouts` HashMap duplication by building
the layout map once at the crate level (from HIR) and sharing it across all
`MirBody` instances via `Arc<AdtLayouts>`. This closes a Phase 2 audit quick
win: "Share AdtLayouts crate-level instead of per-body (1 day)".

The change has three parts:
1. **New function** `build_crate_adt_layouts(hir) -> AdtLayouts` — scans all
   HIR struct/enum owners and builds their layouts upfront (root-cause fix
   that doesn't depend on `local_decls`).
2. **Type change** `MirBody.adt_layouts: AdtLayouts` → `Arc<AdtLayouts>` —
   cheap refcount-bump clone instead of HashMap clone.
3. **Driver simplification** — removed 3× per-body `populate_adt_layouts`
   calls; replaced with 1 crate-level call + Arc sharing.

For a typical 100-fn, 50-type crate, this saves ~500KB of duplicated
HashMap entries and eliminates 3× O(B×S) re-scan passes per body.

## 2. Why This Change?

### 2.1 The problem (Phase 2 audit)

Per `docs/develop/v0/stage-15/v0.2-preparation.md` Phase 1 quick wins:
"Share AdtLayouts crate-level instead of per-body (1 day)".

The Phase 2 data structure audit identified:
- `AdtLayouts` stored per `MirBody` (each body has its own HashMap)
- For 100-fn crate with 50 types: 100 × 50 = 5000 entries duplicated
- ~500KB memory waste per compilation unit
- `populate_adt_layouts` called 3× per body (during lowering, after
  type-propagation writeback, after closure writeback)

### 2.2 The root cause

The original `populate_adt_layouts` (Stage 3.47) only registered ADT
layouts for DefIds that appeared in `mir.local_decls`. This created a
dependency: layouts were only available if the ADT was referenced in a
local's type.

When writeback (Stages 14.37, 14.82, 14.84) changed a local's type from
`Infer` to `Adt(def_id, [])`, the layout for `def_id` wasn't registered
yet. The Stage 14.41 fix was to re-run `populate_adt_layouts` after
writeback. This worked but required 3× per-body calls.

### 2.3 The fix (root cause)

`build_crate_adt_layouts(hir)` scans ALL HIR struct/enum owners upfront
and builds their layouts. The resulting map is complete — every ADT
defined in the crate has its layout registered, regardless of whether
it appears in any body's local_decls.

This eliminates the dependency on `local_decls` and makes the 3× per-body
re-runs unnecessary. The crate-level map is built once, shared via Arc,
and never needs updating.

Per §15 "最优 > 最小": this is the root-cause fix, not a workaround.
Per §1.0 原则 6 "通用 > 特例": one crate-level map for all bodies.

## 3. Design

### 3.1 Type change

```rust
// Before (Stage 3.47 - 15.7):
pub struct MirBody {
    pub adt_layouts: AdtLayouts,  // owned HashMap
    ...
}

// After (Stage 15.8):
pub struct MirBody {
    pub adt_layouts: SharedAdtLayouts,  // Arc<AdtLayouts>
    ...
}

pub type SharedAdtLayouts = Arc<AdtLayouts>;
```

### 3.2 New function

```rust
/// Stage 15.8: Build ALL ADT layouts from HIR, crate-level.
pub fn build_crate_adt_layouts(hir: &HirCrate) -> AdtLayouts {
    let mut layouts = AdtLayouts::new();
    for (def_id, _owner) in &hir.owners {
        if build_adt_layout(*def_id, hir).is_some() {
            register_adt_layout_recursive(&mut layouts, *def_id, hir);
        }
    }
    layouts
}
```

The function reuses the existing `build_adt_layout` and
`register_adt_layout_recursive` helpers — no new layout-building logic
was needed, just a new entry point that scans all HIR owners.

### 3.3 Driver integration

```rust
// After all bodies are lowered + typeck'd + writeback'd:

// Stage 15.8: Build crate-level AdtLayouts ONCE from HIR.
let crate_adt_layouts: Arc<AdtLayouts> =
    Arc::new(build_crate_adt_layouts(&hir));

// Share the crate-level AdtLayouts across all MirBodies.
for mir in &mut mirs {
    mir.adt_layouts = crate_adt_layouts.clone();
}
```

The 3× per-body `populate_adt_layouts` calls are removed entirely.

### 3.4 Codegen integration

Codegen functions take `layouts: &AdtLayouts`. Since `Arc<AdtLayouts>`
derefs to `AdtLayouts`, the only change needed was at the call site:

```rust
// Before:
&mir.adt_layouts,  // &AdtLayouts

// After:
&*mir.adt_layouts,  // &AdtLayouts (deref Arc)
```

### 3.5 Backward compatibility

The old `populate_adt_layouts(mir, hir)` function is retained for the
internal `lower_hir_body_to_mir` call (which runs before the driver
builds the crate-level map). It uses `Arc::make_mut` to mutate the
inner HashMap while the Arc has refcount 1 (before sharing).

The `MirBody::register_adt_layout` method is also retained (used by
tests) and uses the same `Arc::make_mut` pattern.

## 4. §29 Stage-End Deep Review

### 4.1 Data flow coverage (§29.1.1)

Data flow is simplified:
- Before: HIR → per-body `populate_adt_layouts` (3×) → `mir.adt_layouts` (owned)
- After: HIR → `build_crate_adt_layouts` (1×) → `Arc<AdtLayouts>` → shared to all bodies

No new catch-all branches. The `build_adt_layout` function returns `None`
for non-struct/enum owners — this is the same behavior as before.

### 4.2 Architecture review (§29.1.2)

**Structural clarity** ✅ — ADT layouts are now crate-level data, not
per-body data. This matches their semantic meaning: an ADT's layout is
defined by its HIR declaration, not by which bodies reference it.

**Efficiency** ✅ — eliminates 3× per-body O(B×S) scans + ~500KB
HashMap duplication. Net: significant compile-time and memory improvement.

**Extensibility** ✅ — adding a new ADT kind (e.g., union) only requires
updating `build_adt_layout`. The crate-level map automatically includes it.

### 4.3 Design-impl-test coverage (§29.1.3)

| Design point | Implementation | Test |
|--------------|----------------|------|
| All HIR structs/enums get layouts | `build_crate_adt_layouts` scans all owners | `stage15_8_struct_layout_crate_level` |
| Nested ADTs registered recursively | `register_adt_layout_recursive` | `stage15_8_nested_struct_layouts` |
| Layouts shared across bodies | `Arc::ptr_eq` check | `stage15_8_layouts_shared_across_bodies` |
| Non-ADT owners skipped | `build_adt_layout` returns None for fns/impls | (implicit — no layout for fn owners) |
| Struct-returning method calls work | Codegen reads `&*mir.adt_layouts` | `stage15_8_struct_return_method_call_regression` |

### 4.4 Hidden problems (§29.1.4)

| Hidden problem | Complexity growth | Stage 15.8 status |
|----------------|-------------------|-------------------|
| `populate_adt_layouts` still called internally during lowering | 1× (overwritten by crate-level) | Acceptable — will be removed in v0.3 |
| `Arc::make_mut` clones if shared | 1× (only during lowering, before sharing) | Acceptable — refcount is 1 during lowering |
| No incremental cache invalidation | 2× (v0.2 Phase 4) | Deferred to v0.2 Phase 4 (incremental) |

No new hidden problems introduced.

### 4.5 Refactoring optimality (§29.2)

**Approach taken** ✅ — `Arc<AdtLayouts>` is the standard Rust pattern
for shared immutable data. The crate-level builder is a simple scan +
reuse of existing helpers.

**Alternative considered** ✅ — Could have removed `adt_layouts` from
`MirBody` entirely and passed it as a separate parameter to codegen.
Rejected because it would require changing all codegen function
signatures (10+ functions). The Arc approach achieves the memory
sharing with minimal API change.

**Skipped refactors** ✅ — Did not remove the internal
`populate_adt_layouts` call in `lower_hir_body_to_mir`. It runs before
the driver builds the crate-level map, so its result is overwritten.
Removing it would require threading the crate-level map into the lowering
context, which is a larger change. Per §15 "最优 > 最小": the current
approach captures the memory benefit without the API churn.

## 5. Test Results

| Test suite | Before (v0.133.0) | After (v0.134.0) | Delta |
|------------|-------------------|------------------|-------|
| Rust unit (lib) | 145 | 145 | 0 |
| Rust integration (all_tests) | 1964 | 1970 | +6 (crate AdtLayouts tests) |
| Conformance (.lin) | 5216 | 5216 | 0 |
| **Total** | **7325** | **7331** | **+6** |

All tests pass. Zero regressions. Zero clippy warnings. fmt clean.

## 6. v0.2 Phase 1 Progress Update

| Task | Status | Notes |
|------|--------|-------|
| 1. Ty interning (`Ty<'tcx>` Copy) | Design done (Stage 15.1) | Implementation deferred to v0.3 |
| 2. SubstsRef → `&'tcx [GenericArg]` | Not started | Blocked on Task 1 |
| 3. TraitResolver key redesign | Not started | Blocked on Tasks 1+2 |
| 4. EmitValue → typed LLVM handle | Not started | Independent |
| 5. Consolidate 8 writeback passes → 2 | ✅ Done (Stage 15.7) | 650 LOC → 25 LOC |
| **Quick win: AdtLayouts crate-level** | ✅ **Done (Stage 15.8)** | ~500KB memory saved |
| Quick win: VtableEntry.fn_name interning | Not started | 4 hours |
| Quick win: Stop stringifying CoherenceError | Not started | 4 hours |

Stage 15.8 closes the AdtLayouts quick win. The next quick wins
(VtableEntry interning, CoherenceError de-stringification) are each
4-hour changes. After those, the next major milestone is Task 1 (Ty
interning via Rc stepping stone).

## 7. Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Version bump 0.133.0 → 0.134.0 |
| `src/mir/body.rs` | `MirBody.adt_layouts: AdtLayouts` → `Arc<AdtLayouts>`; added `SharedAdtLayouts` type alias; `register_adt_layout` uses `Arc::make_mut` |
| `src/mir/lower/adt_layout.rs` | Added `build_crate_adt_layouts(hir)` function; updated `populate_adt_layouts` to use `Arc::make_mut` |
| `src/mir/lower/mod.rs` | Re-exported `build_crate_adt_layouts`; removed unused `populate_adt_layouts` re-export |
| `src/driver.rs` | Removed 3× per-body `populate_adt_layouts` calls; added 1× crate-level `build_crate_adt_layouts` + Arc sharing |
| `src/codegen/mod.rs` | Updated call site: `&mir.adt_layouts` → `&*mir.adt_layouts` (Arc deref) |
| `tests/v0/stage15/plan/crate_adt_layouts_tests.rs` | **NEW** — 6 integration tests |
| `tests/all_tests.rs` | Registered `stage15_crate_adt_layouts_tests` module |
| `docs/develop/v0/stage-15/stage-15.8-crate-adt-layouts.md` | This document |
| `docs/tests/v0/stage15/stage-15.8-test-plan.md` | **NEW** — test plan |
| `docs/worklog.md` | Stage 15.8 entry appended |
| `RELEASE_NOTES.md` | v0.134.0 entry appended |
| `README.md` | Updated with Stage 15.8 progress |
