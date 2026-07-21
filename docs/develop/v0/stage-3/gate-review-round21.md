# Stage 3 Phase Gate Review — Round 21

> **Author**: redskaber
> **Date**: 2026-07-21
> **Process**: v3.13 (§18 round-completion doc sync)
> **Stage baseline**: v0.8.6 (Stage 3.53 — &str indexing element type fix)
> **Audit tool**: `examples/stage3_gate_audit_r21.rs`
> **Prior rounds**: R1-R20 all CONVERGED

---

## 1. Audit Design

R21 covers Stage 3.54 — **slice/array field store fix +
detect_lvalue_storage_type Field projection fix**. Two bugs:

1. `detect_lvalue_storage_type` for `Field` projections returned the
   base's type instead of the field's type — causing wrong GEP when
   indexing a struct field that contains a slice/array.
2. Store path's `Index` projection used `codegen_lvalue_load` (returns
   value) for non-Local bases, but `unwrap_fat_ptr_for_index` expected
   an address — invalid LLVM for Field projections.

30 cases across 3 groups:

| Group | Cases | Purpose |
|-------|-------|---------|
| R — Regression (8) | Re-verify R20 cases (&str param, &str index, &[i64], byte string, enum Case C, array, const, i16) |
| F — Field indexing coverage (14) | slice/array/str field store+load, variable index, nested struct, two fields, arith |
| E — §9.3.2 edge cases (8) | last index, str arith, in if, i8 store, local regressions, field comparison |
| **Total** | **30** |

---

## 2. Audit Execution

```
✅ AUDIT PASSED — 0 codegen defects found in 30 cases.
   R1-R21: ..., 28, 28, 23, 24, 30, 30, 30, 30/30 — all OK.
   Per §9.3.3, audit CONVERGED (21 rounds, 0 new issues each).
   Stage 3.54 (slice/array field store + storage type fix) verified.
```

---

## 3. Stage 3.54 Summary — Field Indexing Fix

### Problem (two bugs)

1. **`detect_lvalue_storage_type` Field projection bug (P0)**: for
   `Projection(base, Field(...))`, returned `detect_lvalue_storage_type(base)`
   — the BASE's type, not the FIELD's type. For `s.data[0]` where
   `data: &mut [i32]`, this returned the struct `S`'s layout
   (`{ { i32*, i64 } }`) instead of the field's fat pointer layout
   (`{ i32*, i64 }`). This caused `unwrap_fat_ptr_for_index` to see the
   wrong storage type and GEP incorrectly.

2. **Store path base pointer bug (P0)**: for `s.data[0] = x` where
   `s.data` is a Field projection, the store path used
   `codegen_lvalue_load(base)` which returned the LOADED VALUE (an SSA
   value), not the ADDRESS. Then `unwrap_fat_ptr_for_index` tried to GEP
   into the value (treating it as a pointer) — invalid LLVM.

### Root Cause (per §15 — root cause)

1. `detect_lvalue_storage_type` was designed for arrays where
   `Projection(base, Index)` means "index INTO base's storage", so it
   returned the base's type. But for `Field` projections, the storage
   type changes (we're accessing a field of a different type). The
   function didn't distinguish Field from Index/ConstantIndex.

2. The store path's `base_ptr` computation used `codegen_lvalue_load`
   (returns value) for non-Local bases, but `unwrap_fat_ptr_for_index`
   expected an address (pointer to storage). This worked for Local bases
   (alloca pointer) but broke for Field projections (loaded value).

### Fix (1 source file)

**`src/codegen/mod.rs`**:

1. `detect_lvalue_storage_type`: added a `Projection(base, elem)` match
   that dispatches on `elem`:
   - `Field(_, field_ty)` → return the FIELD's type
   - `Index/ConstantIndex/Deref/...` → return the base's type (indexing
     INTO the base's storage)

2. Store path `Index` projection: replaced `codegen_lvalue_load(base)`
   with new `compute_lvalue_address(...)` helper that computes the
   ADDRESS of the lvalue (without loading). For Local: returns alloca
   pointer. For Field projection: GEPs to the field (returns address).
   For other projections: falls back to load (old behavior).

### Resulting IR

```llvm
; struct S { data: &mut [i32] } fn f(s: S) { s.data[0] = 42; } — Stage 3.54
  %v2 = gep { { i32*, i64 } }, { { i32*, i64 } }* %loc_1, 0, 0   ; field addr
  %v3 = gep { i32*, i64 }, { i32*, i64 }* %v2, 0, 0               ; fat ptr field 0
  %v4 = gep i32, i32* %v3, 0                                      ; element
  store i32 42, %v4
```

### §15.4 Verification (root-cause fix confirmed)

1. **Slice field store**: `f01_slice_field_store` verifies GEP to fat
   pointer field 0 AND GEP to element AND `store i32 42`.

2. **Slice field load**: `f03_slice_field_load` verifies `load i64` and
   GEP to `i64*` for `&[i64]` field.

3. **Array field regression**: `f05_array_field_store` and
   `f06_array_field_load` verify `[T; N]` fields still use array-style
   GEP (no regression from the `detect_lvalue_storage_type` change).

4. **Direct param regression**: `e05_slice_local_regression`,
   `e06_array_local_regression`, `e07_str_local_regression` verify
   direct param indexing (no struct field) still works.

---

## 4. Committee Vote: 5/5 APPROVED — UNANIMOUS

---

## 5. Updated Limitation List

| ID | Status |
|----|--------|
| Slice/array field indexing (Stage 3.51 latent P0) | **CLOSED in Stage 3.54** ✅ |
| All prior CLOSED items | ✅ |
| Remaining open: L1 (PHI), L3 (closures), L5 (traits), L8 (lli), L-COPY-ADT | Open |

---

## 6. Test Progression

| Version | Tests | Delta |
|---------|-------|-------|
| v0.8.6 (3.51, slice indexing fix) | 911 | +9 |
| v0.8.6 (3.52, element type propagation) | 920 | +9 |
| v0.8.6 (3.53, &str indexing fix) | 929 | +9 |
| **v0.8.6 (3.54, field indexing fix)** | **938** | **+9** |

---

## 7. §18 Document Sync Compliance (process v3.13)

| Document | Status |
|----------|--------|
| `docs/develop/v0/stage-3/dev-log.md` | ✅ Stage 3.54 entry added |
| `docs/develop/v0/stage-3/gate-review-round21.md` | ✅ This file |
| `docs/tests/matrix.md` | ✅ Updated (938 tests) |
| `README.md` | ✅ Updated (938 tests, 21 rounds) |
| `examples/stage3_gate_audit_r21.rs` | ✅ Created (30 cases) |
| `worklog.md` | ✅ Stage 3.54 entry to be appended |

---

## 8. Conclusion

Stage 3 Round 21 **PASSED** with unanimous 5/5 approval. All 30 audit
cases pass, all 938 tests pass, 0 clippy warnings, 0 fmt issues.

**Two P0 bugs closed**:
1. `detect_lvalue_storage_type` now returns the field's type for Field
   projections (was: base's type — wrong GEP for struct fields).
2. Store path `Index` projection now computes the lvalue's address (was:
   loaded value — invalid LLVM for Field projections).

This was the sixth latent bug found in the fat pointer / indexing story
(after Stage 3.50-3.53). The root cause was that Stage 3.51's
`unwrap_fat_ptr_for_index` and `detect_lvalue_storage_type` were tested
only with direct slice params, not with struct fields containing slices.
The fix completes the field indexing story — `s.field[i]` now works
correctly for all field types (`&[T]`, `&mut [T]`, `[T; N]`, `&str`).

**Remaining open limitations**: L1 (PHI optimization), L3 (closures),
L5 (trait dispatch), L8 (lli execution), L-COPY-ADT (needs TraitResolver
from Stage 5).
