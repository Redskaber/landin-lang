# Stage 15.63 — Recursive Drop (Fields with Drop)

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.188.0 → v0.189.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 3 Task 13**: `impl Drop` + RAII — recursive drop glue

## 1. Executive Summary

Stage 15.63 implements **recursive drop** — when a struct doesn't have
`impl Drop` but has fields that need drop, the drop glue function now
recursively drops each field. Previously, `emit_drop_glue_functions`
only emitted drop glue for types WITH `impl Drop`, causing link errors
for types that need drop (via field recursion) but don't have their own
`impl Drop`.

**Key results**:
- `emit_drop_glue_functions` now iterates ALL types in `type_by_def_id`,
  not just types with `impl Drop`.
- For each type where `ty_needs_drop` returns true, drop glue is emitted.
- For types WITH `impl Drop`: calls user's `Drop::drop`, then recursively
  drops each field needing drop.
- For types WITHOUT `impl Drop` but with Drop fields: recursively drops
  each field (no user drop call).
- Recursive drop uses GEP to compute field addresses, then calls
  `drop_adt_<fieldDefId>` on each field.
- 8 new integration tests covering: single-level, both-have-Drop,
  three-level nesting, multiple Drop fields, mixed Drop/non-Drop fields,
  function returning struct with Drop field, explicit self type.
- All 7560 tests pass (226 lib + 2118 integration + 5216 conformance).

## 2. Root Cause

Before Stage 15.63, `emit_drop_glue_functions` only iterated
`resolver.impl_by_trait_and_type` for entries with trait_name == "Drop".
This means:

- Types WITH `impl Drop`: got drop glue (calling user's `drop` method). ✅
- Types WITHOUT `impl Drop` but with Drop fields: did NOT get drop glue. ❌

But `ty_needs_drop` correctly returns `true` for types with Drop fields
(it recurses through fields). So `elaborate_drops` inserts `Drop`
terminators for these types. When codegen calls `drop_adt_<DefId>`, the
function doesn't exist → **link error**:

```
/usr/bin/ld: undefined reference to `drop_adt_4'
```

## 3. What Was Done

### 3.1 Rewrote `emit_drop_glue_functions`

**Before**: Iterated `impl_by_trait_and_type` for Drop impls only.

**After**: Iterates `type_by_def_id` (all types). For each type:
1. Check `ty_needs_drop(Ty::Adt(def_id, []))` — if false, skip.
2. Check if the type has `impl Drop` (via `resolver.implements`).
3. Emit `drop_adt_<DefId>(ptr %self)`:
   - If has `impl Drop`: call `landin_<Type>_drop(ptr %self)`.
   - For each field needing drop: GEP to field, call `drop_adt_<fieldDefId>`.
4. Build the struct's LLVM type from `AdtLayout::Struct { field_tys }`
   for GEP.

### 3.2 Added `AdtLayouts` parameter

`emit_drop_glue_functions` now takes `&AdtLayouts` (in addition to
`TraitResolver`, `Rodeo`, `fn_name_by_def_id`). The `AdtLayouts` provides
field types for:
- Checking which fields need drop (recursive `ty_needs_drop`).
- Building the struct's LLVM type for GEP.

Both call sites (`codegen_crate` text backend and `codegen_crate_to_module`
LLVM backend) extract `AdtLayouts` from `result.mirs[0].adt_layouts`
(shared via `Arc` across all bodies).

### 3.3 Enum handling

For enums, recursive drop is NOT implemented (deferred to v0.3). Enums
would need `SwitchInt` in the drop glue to check the discriminant and
drop the active variant's payload. For the MVP:
- If an enum has `impl Drop`: the user's `drop` method is called.
- Enum fields are NOT recursively dropped.

## 4. Verification

### 4.1 Quality checks
- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings

### 4.2 Test results
- `cargo test --features llvm-backend --lib` — ✅ 226/226 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2118/2118 PASS
  (was 2110; +8 new recursive drop tests, 2 ignored)
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS

**Total: 7560 tests passing, 0 failures, 0 warnings.**

### 4.3 Runtime verification

**Test 1: Struct without Drop, field with Drop** (previously: link error)
```landin
trait Drop { fn drop(&mut self); }
struct Inner { x: i32 }
impl Drop for Inner { fn drop(&mut self) {} }
struct Outer { inner: Inner }
fn main() -> i32 { let o = Outer { inner: Inner { x: 42 } }; o.inner.x }
```
→ **Exit 42** ✅ (previously: link error `undefined reference to drop_adt_4`)

**Test 2: Three-level nesting** (Outer → Middle → Inner, only Inner has Drop)
```landin
struct Outer { middle: Middle }
struct Middle { inner: Inner }
fn main() -> i32 { let o = Outer { middle: Middle { inner: Inner { x: 42 } } }; o.middle.inner.x }
```
→ **Exit 42** ✅

## 5. Files Modified

### 5.1 `src/codegen/mod.rs`
- **Lines 161-181**: Updated `codegen_crate` to extract `AdtLayouts` and
  pass to `emit_drop_glue_functions`.
- **Lines 320-341**: Updated `codegen_crate_to_module` (LLVM backend) to
  extract `AdtLayouts` and pass to `emit_drop_glue_functions`.
- **Lines 186-353**: Rewrote `emit_drop_glue_functions` — iterates all
  types, checks `ty_needs_drop`, emits recursive drop glue.

### 5.2 `tests/v0/stage15/plan/recursive_drop_tests.rs` (NEW)
- 8 integration tests: single-level, both-have-Drop, three-level, multiple
  Drop fields, mixed Drop/non-Drop, no-regression, function return, explicit
  self type.

### 5.3 `tests/all_tests.rs`
- Registered `stage15_recursive_drop_tests` module.

### 5.4 `Cargo.toml`
- Bumped v0.188.0 → v0.189.0.

## 6. §23 API Naming Standardization Audit

- ✅ `emit_drop_glue_functions` — `emit_` prefix for codegen (rule 7).
- ✅ `ty_needs_drop` — existing public function, reused (rule 5 DRY).
- ✅ No new types introduced (rules 2-3 N/A).
- ✅ No new re-exports (rule 4 N/A).
- ✅ No new `#[deprecated]` items (rule 6 N/A).
- ✅ Function signature change (added `AdtLayouts` parameter) is backward-
  compatible within the crate (the function is `pub(crate)`).

## 7. §25 Deep Review (8 Dimensions)

### D1. Architecture Health — ✅ Excellent
- `emit_drop_glue_functions` now follows the principle "drop glue for ALL
  types needing drop" (§1.0 原則 6 "通用 > 特例").
- Uses existing `ty_needs_drop` for the needs-drop check (DRY).
- Uses existing `AdtLayouts` for field types (§16 compliant — no HIR).

### D2. Technical Debt — ✅ Good (improved)
- Recursive drop for structs: **RESOLVED**.
- Remaining: recursive drop for enums — P2, deferred to v0.3.
- Remaining: struct literal uses `Operand::Copy` for non-Copy types —
  pre-existing bug, causes extra drops of temporaries. Separate from
  recursive drop mechanism.

### D3. Test Coverage — ✅ Excellent
- 8 new integration tests covering all major recursive drop patterns.
- Runtime verification: previously-failing programs now compile and run.
- All 5216 conformance tests pass (no regression).

### D4. Next Phase Readiness — ✅ Excellent
- Task 13 is now even more complete (recursive drop + drop order + double-drop).
- Task 12 (Lifetime elision) is the next ready task.

### D5. Design Rationality — ✅ Excellent
- The approach matches rustc: drop glue is emitted for all types needing
  drop, not just types with `impl Drop`.
- Recursive drop via GEP + call is the standard LLVM approach.
- Enum drop is deferred (requires SwitchInt) — pragmatic MVP choice.

### D6. Performance — ✅ Excellent
- `emit_drop_glue_functions`: O(T × F) where T = types, F = avg fields.
- Each drop glue function is O(D) where D = fields needing drop.
- No measurable compile-time impact.

### D7. Documentation — ✅ Excellent
- This stage doc (15.63) with root cause + design rationale.
- Inline doc comments for the rewritten `emit_drop_glue_functions`.
- Test plan doc (see `docs/tests/v0/stage15/stage-15.63-test-plan.md`).

### D8. Test Path Coverage — ✅ Excellent
- All recursive drop paths tested:
  - Struct without Drop, field with Drop: `stage15_63_recursive_drop_outer_no_drop_inner_drop`.
  - Both have Drop: `stage15_63_recursive_drop_both_have_drop`.
  - Three-level nesting: `stage15_63_recursive_drop_three_levels`.
  - Multiple Drop fields: `stage15_63_recursive_drop_multiple_drop_fields`.
  - Mixed Drop/non-Drop: `stage15_63_recursive_drop_mixed_fields`.
  - No regression: `stage15_63_no_drop_all_primitives_no_regression`.

## 8. Committee Vote: GO

**Decision**: Stage 15.63 is **COMPLETE**. Recursive drop works correctly
for structs. Enum recursive drop deferred to v0.3.

## 9. v0.2 Phase 3 Status (Updated)

| Task | Status | Description |
|------|--------|-------------|
| Task 11 (Monomorphization) | ⏳ Blocked | Needs Task 3 |
| Task 12 (Lifetime elision) | ⏳ Ready | Next task |
| **Task 13 (impl Drop + RAII)** | **✅ COMPLETE+** | **+ recursive drop (this stage)** |
| Task 14 (Object safety) | ⏳ Blocked | Needs Task 3 |

## 10. Remaining Work (Deferred to v0.3)

| Item | Effort | Priority |
|------|--------|----------|
| Recursive drop for enums (SwitchInt) | 1-2 days | P2 |
| Struct literal Copy→Move for non-Copy types | 0.5 day | P2 |
| Full drop flags (runtime tracking) | 2-3 days | P2 |
| Partial move handling | 1 day | P2 |
| `Box<T>` in prelude | 2 days | P2 |
