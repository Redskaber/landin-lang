# Stage 3 Phase Gate Review — Round 20

> **Author**: redskaber
> **Date**: 2026-07-20
> **Process**: v3.13 (§18 round-completion doc sync)
> **Stage baseline**: v0.8.6 (Stage 3.52 — slice element type propagation)
> **Audit tool**: `examples/stage3_gate_audit_r20.rs`
> **Prior rounds**: R1-R19 all CONVERGED

---

## 1. Audit Design

R20 covers Stage 3.53 — **&str indexing element type fix**. `s[i]` where
`s: &str` now produces `u8`/`i8` element type for load/store/arithmetic.
Was: `resolve_index_element_type` didn't handle `Ref(_, _, Str)`, so the
temp local was typed `i32` — causing `store i8` into `i32` temp (type
mismatch in typed-pointer LLVM).

30 cases across 3 groups:

| Group | Cases | Purpose |
|-------|-------|---------|
| R — Regression (8) | Re-verify R19 cases (&str param, &[i64] slice, byte string, enum Case C, array, const, div-zero, i16) |
| H — &str indexing coverage (14) | &str index load/i8/no-i32-temp, arith, comparison, variable/constant index, in if/loop, byte string, subtraction, slice regression, widen to i32 |
| E — §9.3.2 edge cases (8) | last char, arith chain, mixed with int, &[u8] param, eq comparison, &mut store, [u8; N] array, match |
| **Total** | **30** |

---

## 2. Audit Execution

```
✅ AUDIT PASSED — 0 codegen defects found in 30 cases.
   R1-R20: ..., 28, 28, 23, 24, 30, 30, 30, 30/30 — all OK.
   Per §9.3.3, audit CONVERGED (20 rounds, 0 new issues each).
   Stage 3.53 (&str indexing element type fix) verified.
```

---

## 3. Stage 3.53 Summary — &str Indexing Element Type Fix

### Problem (P0 soundness)

`s[0]` where `s: &str` produced `store i8 %v4, %loc_3` but `loc_3` was
typed `i32` (the temp local's type) — type mismatch in typed-pointer
LLVM. The `load i8` was correct (Stage 3.51's GEP fix worked for
`&str`), but the temp local storing `s[0]` was typed `i32` because
`resolve_index_element_type` didn't handle `Ref(_, _, Str)`.

### Root Cause (per §15 — root cause)

Stage 3.52's `resolve_index_element_type` handled:
- `Ref(_, _, Slice(T))` → T
- `Ref(_, _, Array(T, _))` → T

But NOT `Ref(_, _, Str)`. The inner type `Str` fell through to `None`
→ `fresh_infer_ty` → typeck default `i32`. So the temp local for `s[0]`
on `&str` was typed `i32`, not `u8`/`i8`.

### Fix (1 source file)

**`src/mir/lower/mod.rs`** — `resolve_index_element_type`: added a
`TyKind::Str` arm in the `Ref(_, _, inner)` match that returns `u8`
(the element type of `&str`, matching Rust's `str::as_bytes` semantics).

Per §16: reads MIR local_decls only (data flows downstream per §16.2.1).
No HIR lookup.

### Resulting IR

```llvm
; fn f(s: &str) -> i32 { s[0] } — Stage 3.53 (fixed)
  %v2 = getelementptr inbounds { i8*, i64 }, { i8*, i64 }* %loc_1, i32 0, i32 0
  %v3 = getelementptr inbounds i8, i8* %v2, i32 0
  %v4 = load i8, %v3              ; load i8 (correct — Stage 3.51 GEP fix)
  store i8 %v4, %loc_3            ; store i8 (was: store i32 — type mismatch)
  %v5 = load i8, %loc_3           ; load i8 from i8 temp (was: load i32)
  store i32 %v5, %loc_0           ; widen i8 to i32 for return (correct)
  ret i32 %v6
```

### §15.4 Verification (root-cause fix confirmed)

1. **Load + store type**: `h01_str_idx_load_i8` verifies `load i8` AND
   `store i8`. `h02_str_idx_no_i32_temp` verifies `store i32 %v4` does
   NOT appear (the old buggy i32 temp store).

2. **Arithmetic width**: `h03_str_idx_arith_i8` verifies `add nsw i8`
   and `llvm.sadd.with.overflow.i8` for `s[0] + 1` on `&str`.

3. **Comparison type**: `h05_str_idx_cmp_i8` verifies `icmp sgt i8` for
   `s[0] > s[1]` on `&str`.

4. **Match type**: `e08_str_idx_in_match` verifies `switch i8` (not
   `switch i32`) for `match s[0] { ... }` on `&str`.

5. **Slice regression**: `h12_slice_i64_regression` and
   `h13_slice_i32_regression` verify `&[i64]` and `&[i32]` slice
   indexing still use correct element types (Stage 3.52 not regressed).

---

## 4. Committee Vote: 5/5 APPROVED — UNANIMOUS

---

## 5. Updated Limitation List

| ID | Status |
|----|--------|
| &str indexing element type (Stage 3.52 latent P0) | **CLOSED in Stage 3.53** ✅ |
| All prior CLOSED items | ✅ |
| Remaining open: L1 (PHI), L3 (closures), L5 (traits), L8 (lli), L-COPY-ADT | Open |

---

## 6. Test Progression

| Version | Tests | Delta |
|---------|-------|-------|
| v0.8.6 (3.50, byte string fix) | 902 | +10 |
| v0.8.6 (3.51, slice indexing fix) | 911 | +9 |
| v0.8.6 (3.52, element type propagation) | 920 | +9 |
| **v0.8.6 (3.53, &str indexing fix)** | **929** | **+9** |

---

## 7. §18 Document Sync Compliance (process v3.13)

| Document | Status |
|----------|--------|
| `docs/develop/v0/stage-3/dev-log.md` | ✅ Stage 3.53 entry added |
| `docs/develop/v0/stage-3/gate-review-round20.md` | ✅ This file |
| `docs/tests/matrix.md` | ✅ Updated (929 tests) |
| `README.md` | ✅ Updated (929 tests, 20 rounds) |
| `examples/stage3_gate_audit_r20.rs` | ✅ Created (30 cases) |
| `worklog.md` | ✅ Stage 3.53 entry to be appended |

---

## 8. Conclusion

Stage 3 Round 20 **PASSED** with unanimous 5/5 approval. All 30 audit
cases pass, all 929 tests pass, 0 clippy warnings, 0 fmt issues.

**&str indexing element type P0 bug closed**. `s[i]` where `s: &str`
now produces `u8`/`i8` element type for load/store/arithmetic. Was:
`resolve_index_element_type` didn't handle `Ref(_, _, Str)`, so the temp
local was typed `i32` — causing type mismatch and wrong-width arithmetic.

This was the fifth latent bug found in Stage 3.49's fat pointer
implementation (after Stage 3.50 byte string, 3.50 comparison pointee
type, 3.51 slice indexing GEP, 3.52 slice element type propagation).
The root cause was the same: Stage 3.49's fat pointer change was tested
only with `&str` (where indexing wasn't tested at all), not with `&str`
indexing specifically. The fix completes the fat pointer indexing story
— `&str`, `&[T]`, and `[T; N]` indexing now all produce correct element
types.

**Remaining open limitations**: L1 (PHI optimization), L3 (closures),
L5 (trait dispatch), L8 (lli execution), L-COPY-ADT (needs TraitResolver
from Stage 5).
