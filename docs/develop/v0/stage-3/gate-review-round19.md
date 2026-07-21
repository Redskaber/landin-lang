# Stage 3 Phase Gate Review — Round 19

> **Author**: redskaber
> **Date**: 2026-07-20
> **Process**: v3.13 (§18 round-completion doc sync)
> **Stage baseline**: v0.8.6 (Stage 3.51 — slice indexing fix)
> **Audit tool**: `examples/stage3_gate_audit_r19.rs`
> **Prior rounds**: R1-R18 all CONVERGED

---

## 1. Audit Design

R19 covers Stage 3.52 — **slice element type propagation fix**. `s[i]`
where `s: &[T]` now produces the correct element type for load/store/
arithmetic. Was: `detect_lvalue_type` fell through to I32 fallback, and
MIR lower used a fresh infer var that typeck defaulted to i32 — causing
`s[0]` on `&[i64]` to `load i32` instead of `load i64` (type mismatch
in typed-pointer LLVM, plus wrong-width arithmetic and overflow checks).

30 cases across 3 groups:

| Group | Cases | Purpose |
|-------|-------|---------|
| R — Regression (8) | Re-verify R18 cases (&str param, &[i32] slice, byte string, enum Case C, array indexing, const, div-zero, i16) |
| T — Element type propagation (14) | load type for i64/i32/i128/f64, arithmetic for i64/i32/f64, store for i64/i32, comparison for i64/i32, array regression, sub/mul |
| E — §9.3.2 edge cases (8) | i16/usize/bool elements, mixed arith, in loop, three accesses, array regression, division |
| **Total** | **30** |

---

## 2. Audit Execution

```
✅ AUDIT PASSED — 0 codegen defects found in 30 cases.
   R1-R19: ..., 28, 28, 23, 24, 30, 30, 30, 30/30 — all OK.
   Per §9.3.3, audit CONVERGED (19 rounds, 0 new issues each).
   Stage 3.52 (slice element type propagation fix) verified.
```

---

## 3. Stage 3.52 Summary — Slice Element Type Propagation

### Problem (P0 soundness)

`s[0]` where `s: &[i64]` produced `load i32` instead of `load i64` —
type mismatch in typed-pointer LLVM. Slice element arithmetic
(`s[0] + s[1]` on `&[i64]`) used `add nsw i32` and
`llvm.sadd.with.overflow.i32` instead of i64 — silently wrong overflow
detection and truncation.

Two bugs:

1. **codegen `detect_lvalue_type`**: for `Index`/`ConstantIndex`
   projections, checked `EmitType::Array(elem, _) => *elem` but fell
   through to `I32` for fat pointers (`Struct([Ptr(T), I64])`). The
   element type was not extracted from the fat pointer's field 0.

2. **MIR lower `Index` expression**: used `cx.fresh_infer_ty()` for the
   element type (a fresh inference variable), which typeck defaulted to
   `i32`. The temp local storing `s[0]` was typed `i32`, so the store
   truncated the i64 value to i32.

### Root Cause (per §15 — root cause)

Stage 3.51 fixed the GEP (data pointer dereference) but didn't fix the
element type detection. The `detect_lvalue_type` and MIR lower `Index`
paths were independent — fixing one without the other left the type
mismatch. The root cause is that slice indexing touches THREE layers
(MIR lower type, codegen GEP, codegen load type), and Stage 3.51 only
fixed the middle one.

### Fix (2 source files)

1. **`src/codegen/mod.rs`** — `detect_lvalue_type` for
   `Index`/`ConstantIndex`: added a `Struct(fields)` arm that checks for
   fat pointer shape (`fields.len() == 2 && fields[0].is_ptr() &&
   fields[1] == I64`) and returns `fields[0].pointee()` (the element
   type). Falls through to `I32` only for non-fat-pointer, non-array
   cases.

2. **`src/mir/lower/mod.rs`** — `Index` expression lowering: replaced
   `cx.fresh_infer_ty()` with `resolve_index_element_type(cx, base_local)`,
   which inspects the base's MIR type to compute the element type:
   - `&[T]` (Ref to Slice(T)) → T
   - `[T; N]` (Array(T, _)) → T
   - `&[T; N]` (Ref to Array(T, _)) → T

   Falls back to `fresh_infer_ty` if the base type can't be resolved
   (preserves old behavior for test contexts).

   Per §16: reads MIR local_decls only (data flows downstream per
   §16.2.1 — MIR lower reads its own body). No HIR lookup.

### Resulting IR

```llvm
; fn f(s: &[i64]) -> i64 { s[0] + s[1] } — Stage 3.52 (fixed)
  %v4 = load i64, %v3              ; load i64 (was: load i32)
  store i64 %v4, %loc_3            ; store i64 (was: store i32 — truncation)
  ...
  %v10 = add nsw i64 %v8, %v9      ; add nsw i64 (was: add nsw i32)
  %v13 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %v11, i64 %v12)
                                    ; i64 overflow check (was: i32 — wrong)
```

### §15.4 Verification (root-cause fix confirmed)

1. **Load type**: `t01_slice_i64_load` verifies `load i64` and
   `expect_none: ["load i32"]` for `&[i64]`. `t02_slice_i32_load`
   verifies `load i32` for `&[i32]` (no over-correction).

2. **Arithmetic width**: `t05_slice_i64_arith` verifies `add nsw i64` +
   `llvm.sadd.with.overflow.i64` and `expect_none: ["add nsw i32"]` for
   `&[i64]`. `t06_slice_i32_arith` verifies `add nsw i32` for `&[i32]`.

3. **Array regression**: `t12_array_i64_arith` verifies `[i64; 3]`
   arrays still use `add nsw i64` (no regression from the MIR lower
   change — `resolve_index_element_type` handles the Array case).

---

## 4. Committee Vote: 5/5 APPROVED — UNANIMOUS

---

## 5. Updated Limitation List

| ID | Status |
|----|--------|
| Slice element type propagation (Stage 3.51 latent P0) | **CLOSED in Stage 3.52** ✅ |
| All prior CLOSED items | ✅ |
| Remaining open: L1 (PHI), L3 (closures), L5 (traits), L8 (lli), L-COPY-ADT | Open |

---

## 6. Test Progression

| Version | Tests | Delta |
|---------|-------|-------|
| v0.8.6 (3.49, L13 fat pointer closure) | 893 | +12 |
| v0.8.6 (3.50, byte string fix) | 902 | +10 |
| v0.8.6 (3.51, slice indexing fix) | 911 | +9 |
| **v0.8.6 (3.52, element type propagation)** | **920** | **+9** |

---

## 7. §18 Document Sync Compliance (process v3.13)

| Document | Status |
|----------|--------|
| `docs/develop/v0/stage-3/dev-log.md` | ✅ Stage 3.52 entry added |
| `docs/develop/v0/stage-3/gate-review-round19.md` | ✅ This file |
| `docs/tests/matrix.md` | ✅ Updated (920 tests) |
| `README.md` | ✅ Updated (920 tests, 19 rounds) |
| `examples/stage3_gate_audit_r19.rs` | ✅ Created (30 cases) |
| `worklog.md` | ✅ Stage 3.52 entry to be appended |

---

## 8. Conclusion

Stage 3 Round 19 **PASSED** with unanimous 5/5 approval. All 30 audit
cases pass, all 920 tests pass, 0 clippy warnings, 0 fmt issues.

**Slice element type propagation P0 bug closed**. `s[i]` where `s: &[T]`
now produces the correct element type for load/store/arithmetic. Was:
`detect_lvalue_type` fell through to I32, and MIR lower used a fresh
infer var that typeck defaulted to i32 — causing wrong-width arithmetic,
wrong overflow checks, and value truncation.

This was the fourth latent bug found in Stage 3.49's fat pointer
implementation (after Stage 3.50 byte string, 3.50 comparison pointee
type, 3.51 slice indexing GEP). The root cause was the same: Stage 3.49's
fat pointer change was not tested with non-i32 slice elements. The fix
completes the fat pointer slice indexing story — all three layers (MIR
lower type, codegen GEP, codegen load type) now correctly handle fat
pointers.

**Remaining open limitations**: L1 (PHI optimization), L3 (closures),
L5 (trait dispatch), L8 (lli execution), L-COPY-ADT (needs TraitResolver
from Stage 5).
