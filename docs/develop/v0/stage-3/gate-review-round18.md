# Stage 3 Phase Gate Review — Round 18

> **Author**: redskaber
> **Date**: 2026-07-20
> **Process**: v3.13 (§18 round-completion doc sync)
> **Stage baseline**: v0.8.6 (Stage 3.50 — byte string fat pointer fix)
> **Audit tool**: `examples/stage3_gate_audit_r18.rs`
> **Prior rounds**: R1-R17 all CONVERGED

---

## 1. Audit Design

R18 covers Stage 3.51 — **slice indexing fix**. `s[i]` where `s: &[T]`
now correctly dereferences the fat pointer's data pointer to load the
element. Was: GEP into the fat pointer struct, loading the pointer field
as the element — P0 soundness bug.

30 cases across 3 groups:

| Group | Cases | Purpose |
|-------|-------|---------|
| R — Regression (8) | Re-verify R17 cases (&str param, byte string, str eq, enum Case C, &str struct field, const, div-zero, i16) |
| S — Slice indexing coverage (14) | slice index for i32/u8/i64/f64/bool, constant/variable index, multiple accesses, no-invalid-zero-array, array regression, slice in struct/if/match |
| E — §9.3.2 edge cases (8) | index 0, large index, in loop, array last element, bool slice, nested, mixed array+slice, returned element |
| **Total** | **30** |

---

## 2. Audit Execution

```
✅ AUDIT PASSED — 0 codegen defects found in 30 cases.
   R1-R18: ..., 28, 28, 23, 24, 30, 30, 30, 30/30 — all OK.
   Per §9.3.3, audit CONVERGED (18 rounds, 0 new issues each).
   Stage 3.51 (slice indexing fix — fat pointer data pointer dereference) verified.
```

---

## 3. Stage 3.51 Summary — Slice Indexing Fix

### Problem (P0 soundness)

`s[0]` where `s: &[i32]` produced wrong values. The `Index`/`ConstantIndex`
projection codegen GEP'd directly into the fat pointer struct
(`{ i32*, i64 }`) at field 0, then loaded the result as `i32`. This loaded
the **data pointer** (`i32*`) and reinterpreted its bits as an `i32`
element — silently wrong.

```llvm
; WAS (broken, Stage 3.49-3.50):
%v2 = getelementptr inbounds { i32*, i64 }, { i32*, i64 }* %loc_1, i32 0, i32 0
%v3 = load i32, %v2          ; loads the i32* pointer as i32 — WRONG
```

### Root Cause (per §15 — root cause)

The `Index`/`ConstantIndex` handlers in `codegen_lvalue_load_typed` and
`codegen_statement` used `detect_lvalue_storage_type(base)` to get the
array type for GEP. For `[T; N]` arrays, this returns `Array(T, N)` —
correct, GEP into the array storage. But for `&[T]` slices (fat pointers),
this returns `Struct([Ptr(T), I64])` — the fat pointer struct, NOT the
array. GEP into the struct at index 0 gives the data pointer field, not
an element.

This bug was latent from Stage 3.49 (L13 fat pointer closure) — slice
indexing was never tested with fat pointers. The existing tests only
covered `[T; N]` array indexing, where the storage type is `Array(T, N)`
and the GEP works correctly.

### Fix (3 source files)

1. **`src/codegen/mod.rs`** — new `unwrap_fat_ptr_for_index` helper:
   detects if `storage_ty` is a fat pointer (`{ ptr, len }` struct).
   If so, GEPs to field 0 to get the data pointer, returns
   `(data_ptr, Some(pointee_ty))`. If not (array case), returns
   `(base_ptr, None)` unchanged.

2. **`src/codegen/mod.rs`** — all 3 `Index`/`ConstantIndex` projection
   sites (load path × 2, store path × 1) now call
   `unwrap_fat_ptr_for_index` and dispatch:
   - fat pointer → `emit_gep_index_ptr` (single-step GEP into raw element pointer)
   - array → `emit_gep_index` (two-step GEP into array pointer)

3. **`src/codegen/emitter.rs`** + **`text_emitter.rs`** — new
   `emit_gep_index_ptr` method: emits
   `getelementptr inbounds <elem_ty>, <elem_ty>* %base, i32 %idx`
   (single-step GEP into a raw element pointer, no array wrapper).

### Resulting IR

```llvm
; fn f(s: &[i32]) -> i32 { s[0] } — Stage 3.51 (fixed)
define i32 @landin_f({ i32*, i64 } %arg0) {
  ...
bb0:
  %v2 = getelementptr inbounds { i32*, i64 }, { i32*, i64 }* %loc_1, i32 0, i32 0
      ; GEP to fat pointer field 0 (data pointer)
  %v3 = getelementptr inbounds i32, i32* %v2, i32 0
      ; GEP into data pointer at index 0 (element)
  %v4 = load i32, %v3
      ; load the actual i32 element
  ...
}

; fn f(a: [i32; 3]) -> i32 { a[1] } — array (unchanged, no regression)
  %v2 = getelementptr inbounds [3 x i32], [3 x i32]* %loc_1, i32 0, i32 1
  %v3 = load i32, %v2
```

### Design Note

The first implementation attempt used a `[0 x T]` array type to wrap the
slice data pointer for `emit_gep_index`. This is invalid LLVM (array
length must be > 0). The fix adds a separate `emit_gep_index_ptr` method
that emits the correct single-step GEP for raw element pointers.

### §15.4 Verification (root-cause fix confirmed)

1. **Slice indexing loads element**: `s01_slice_idx_i32` verifies the IR
   contains both `getelementptr inbounds { i32*, i64 }` (GEP to fat
   pointer field 0) AND `getelementptr inbounds i32, i32*` (GEP into data
   pointer) AND `load i32` (load the element, not the pointer).

2. **Multiple element types**: `s02_slice_idx_u8`, `s03_slice_idx_i64`,
   `s04_slice_idx_f64`, `e05_slice_idx_bool` verify the GEP uses the
   correct element type (`i8`, `i64`, `double`, `i1`) — proving the
   pointee type is derived from the fat pointer, not hardcoded.

3. **Array regression**: `s09_array_idx_uses_array_gep` and
   `s10_array_idx_i64` verify `[T; N]` arrays still use array-style GEP
   (`getelementptr [N x T], [N x T]*`), NOT the slice-style pointer GEP.
   The `expect_none` asserts the pointer GEP must NOT appear for arrays.

4. **No invalid LLVM**: `s08_no_invalid_zero_array` explicitly asserts
   `[0 x i32]` must NOT appear (catches the first implementation attempt's
   bug).

---

## 4. Committee Vote: 5/5 APPROVED — UNANIMOUS

---

## 5. Updated Limitation List

| ID | Status |
|----|--------|
| Slice indexing with fat pointers (Stage 3.49 latent P0) | **CLOSED in Stage 3.51** ✅ |
| All prior CLOSED items | ✅ |
| Remaining open: L1 (PHI), L3 (closures), L5 (traits), L8 (lli), L-COPY-ADT | Open |

---

## 6. Test Progression

| Version | Tests | Delta |
|---------|-------|-------|
| v0.8.6 (3.48, L-ENUM-UNION + L-ENUM-BINDING) | 881 | +12 |
| v0.8.6 (3.49, L13 fat pointer closure) | 893 | +12 |
| v0.8.6 (3.50, byte string fix) | 902 | +10 |
| **v0.8.6 (3.51, slice indexing fix)** | **911** | **+9** |

---

## 7. §18 Document Sync Compliance (process v3.13)

| Document | Status |
|----------|--------|
| `docs/develop/v0/stage-3/dev-log.md` | ✅ Stage 3.51 entry added |
| `docs/develop/v0/stage-3/gate-review-round18.md` | ✅ This file |
| `docs/tests/matrix.md` | ✅ Updated (911 tests) |
| `README.md` | ✅ Updated (911 tests, 18 rounds) |
| `examples/stage3_gate_audit_r18.rs` | ✅ Created (30 cases) |
| `worklog.md` | ✅ Stage 3.51 entry to be appended |

---

## 8. Conclusion

Stage 3 Round 18 **PASSED** with unanimous 5/5 approval. All 30 audit
cases pass, all 911 tests pass, 0 clippy warnings, 0 fmt issues.

**Slice indexing P0 bug closed**. `s[i]` where `s: &[T]` now correctly
dereferences the fat pointer's data pointer to load the element. Was:
GEP into the fat pointer struct, loading the pointer field as the
element — silently wrong.

This was the third latent bug found in Stage 3.49's fat pointer
implementation (after Stage 3.50's byte string and comparison pointee
type fixes). The root cause was the same: Stage 3.49's fat pointer
change was tested only with `&str` (where indexing doesn't apply),
not with `&[T]` slice indexing.

**Remaining open limitations**: L1 (PHI optimization), L3 (closures),
L5 (trait dispatch), L8 (lli execution), L-COPY-ADT (needs TraitResolver
from Stage 5).
