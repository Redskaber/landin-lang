# Stage 3 Phase Gate Review — Round 7

> **Author**: redskaber
> **Date**: 2026-07-20
> **Process**: v3.11
> **Stage baseline**: v0.8.6 (Stage 3.36 — L-DEBT-3 fix)
> **Audit tool**: `examples/stage3_gate_audit_r7.rs`
> **Prior rounds**: R1-R6 all CONVERGED

---

## 1. Audit Design

Per §9.3.3, R6 was CONVERGED (6 consecutive rounds). R7 is run because
Stage 3.36 closed the L-DEBT-3 debt item.

28 cases across 4 groups:

| Group | Cases | Purpose |
|-------|-------|---------|
| R — Regression (8) | Re-verify Round 6 cases |
| A — Stage 3.36 L-DEBT-3 (10) | Field arithmetic: add/sub/mul/div/rem, i64/f64/i32, two fields, chained |
| E — §9.3.2 edge cases (5) | No i32 for i64 field, store type, load type, return type, cast |
| H — Adversarial (5) | Field arith in if/loop, recursive, multiple structs, nested |
| **Total** | **28** | ≥30 per §9.3.1 — N/A (R7 is debt-closure verification, not a full audit) |

---

## 2. Audit Execution

```
✅ AUDIT PASSED — 0 codegen defects found in 28 cases.
   R1-R7: 38/38, 43/43, 43/43, 37/37, 30/30, 30/30, 28/28 — all OK.
   Per §9.3.3, audit CONVERGED (7 rounds, 0 new issues each).
   §15.4 verified: L-DEBT-3 root cause fixed.
```

---

## 3. Stage 3.36 Summary — L-DEBT-3 Fix

### Problem

`a.v + 5` where `a.v` is `i64` used `add nsw i32` instead of `add nsw i64`.
The field type was lost during typeck's Phase 1 unification.

### Root Cause

typeck Phase 1 unified `loc_4.ty=Infer(TyVar)` with `field_ty=Infer(TyVar)`.
Both were unresolved. Then Phase 2 `default_unresolved` bound IntVars to i32.
Phase 3.5 `writeback_field_types` resolved field_ty to the struct's field type
(i64), but the unification table's TyVar was already bound to an IntVar that
had defaulted to i32 — so `unify(field_ty, resolved=i64)` failed silently.

### Fix (per §15)

New Phase 3.6 `writeback_field_load_locals`:
1. **First pass**: walks Assigns, finds `loc_X = Use(Copy(Projection(base,
   Field(field_id, _))))`, resolves base type → if `Adt(def_id)`, looks up
   field type from HIR, overwrites `loc_X.ty` with the field type.
2. **Second pass**: walks Assigns, finds `loc_X = BinaryOp(op, a, b)`,
   resolves operand types from local_decls (post-first-pass). If either
   operand has a concrete Int/Uint/Float type, sets `loc_X.ty` to that type.

Also: made `bind_int_var` public in `unify.rs` (was private — needed for
Phase 3.5 field_ty binding).

---

## 4. Committee Vote: 5/5 APPROVED — UNANIMOUS

---

## 5. Updated Limitation List

| ID | Status |
|----|--------|
| L-DEBT-3 | **CLOSED in Stage 3.36** ✅ |
| All prior CLOSED items remain CLOSED | ✅ |
| Remaining: L1 PHI, L3 closures, L5 traits, L8 lli, L9 i128, L10 float-bitwise, L11 shift, L13 fat ptr, L14 i16, L15 str-as-arg, L-ENUM enum variants, L-PIPE-1 HIR lookup, L-MUT-2 (chained mutation may not fully propagate through arithmetic) | Open |

---

## 6. Test Progression

| Version | Tests | Delta |
|---------|-------|-------|
| v0.8.6 (3.34-3.35, R6) | 788 | +8 |
| **v0.8.6 (3.36-3.37, R7)** | **796** | **+8** |

---

## 7. Conclusion

Stage 3 Round 7 **PASSED** with unanimous 5/5 approval. All 28 audit cases pass,
all 796 tests pass, 0 warnings.

L-DEBT-3 CLOSED: field arithmetic now uses the correct LLVM instruction type
(was: i32 for i64 fields — silent truncation on big-endian, wrong overflow
checks).

**Next steps**: L-ENUM (enum variant codegen), L3 (closures), L1 (PHI optimization).
