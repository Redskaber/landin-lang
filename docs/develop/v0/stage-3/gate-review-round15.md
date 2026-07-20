# Stage 3 Phase Gate Review — Round 15

> **Author**: redskaber
> **Date**: 2026-07-20
> **Process**: v3.13 (§18 round-completion doc sync)
> **Stage baseline**: v0.8.6 (Stage 3.47 — L-PIPE-1 closure via AdtLayout side-table)
> **Audit tool**: `examples/stage3_gate_audit_r15.rs`
> **Prior rounds**: R1-R14 all CONVERGED

---

## 1. Audit Design

R15 covers Stage 3.48 — **L-ENUM-UNION + L-ENUM-BINDING closure**. This
stage closes two soundness bugs:

1. **L-ENUM-UNION**: enum storage layout used "first non-empty variant
   payload" (Stage 3.38 behavior). For `enum E { A, B(i32), C(i64) }`,
   storage was `{ i32, i32 }` (discr + B's i32). Constructing `E::C(42)`
   would write the i64 payload into the i32 slot — silent memory
   corruption.

2. **L-ENUM-BINDING** (hidden P0): `Opt::Some(x) => x` pattern matching
   allocated a local for `x` but never assigned it — the binding read
   uninitialized memory. Pre-existing since Stage 3.40 (L-ENUM-MATCH),
   never caught because the existing test only asserted `switch i32` is
   present, not that the binding actually receives the payload.

30 cases across 3 groups:

| Group | Cases | Purpose |
|-------|-------|---------|
| R — Regression (8) | Re-verify R14 cases (struct, nested struct, i128 field, &str field, const, i16, div-zero, float bitwise) |
| U — L-ENUM-UNION + L-ENUM-BINDING coverage (14) | Case C layout/ctor/match, binding extraction (Case B & C), multi-field variant, mixed types, struct variant binding, Case A/B regression |
| E — §9.3.2 edge cases (8) | 3 non-empty variants, bool payload, enum in struct, enum return, enum param, match with wildcard, enum in tuple, two enums in one fn |
| **Total** | **30** |

Per §9.3.1 (≥30 cases) and §9.3.2 (≥5 boundary cases) — both satisfied.

---

## 2. Audit Execution

```
✅ AUDIT PASSED — 0 codegen defects found in 30 cases.
   R1-R15: ..., 28, 28, 23, 24, 30, 30, 30/30 — all OK.
   Per §9.3.3, audit CONVERGED (15 rounds, 0 new issues each).
   Stage 3.48 (L-ENUM-UNION + L-ENUM-BINDING closure) verified.
```

---

## 3. Stage 3.48 Summary — L-ENUM-UNION + L-ENUM-BINDING Closure

### Problem (two bugs)

#### Bug 1: L-ENUM-UNION (soundness — silent memory corruption)

`mir_type_to_emit_type_with_layouts` for `AdtLayout::Enum` only included
the **first non-empty variant's** payload fields in the storage layout.
For `enum E { A, B(i32), C(i64) }`:

- `variant_payloads = [[], [i32], [i64]]`
- Storage layout (Stage 3.47): `{ i32 (discr), i32 (B's payload) }` — only 2 fields
- `E::C(42)` construction would `insertvalue { i32, i32 } undef, i64 42, 1`
  — writing an 8-byte i64 into the 4-byte i32 slot at field 1, silently
  overflowing into adjacent stack memory.

The `AdtLayout::Enum { variant_payloads }` data structure (introduced in
Stage 3.47) already stored ALL variants' payloads, but codegen was
discarding them.

#### Bug 2: L-ENUM-BINDING (P0 soundness — reading uninitialized memory)

`collect_pat_bindings_for_mir` (in `src/mir/lower/mod.rs`) allocated
locals for `Ident` sub-patterns in `TupleStruct`/`Struct` patterns, but
generated **no projection** to extract the enum's payload. For
`Opt::Some(x) => x`:

- A local for `x` was allocated with a fresh inferred type.
- No assignment was generated to populate `x` from the scrutinee.
- The arm body `x` read whatever was on the stack — uninitialized memory.

This bug existed since Stage 3.40 (L-ENUM-MATCH) but was never caught
because `codegen_enum_match_two_variants` only asserted `switch i32` is
present (verifying the discriminant switch), not that the binding
actually receives the payload.

### Root Cause (per §15 — root cause)

1. **L-ENUM-UNION**: codegen's `mir_type_to_emit_type_with_layouts` used
   `variant_payloads.iter().find(|v| !v.is_empty())` to pick only the
   first non-empty variant. The fix is to iterate ALL variants and
   flatten their payload fields.

2. **L-ENUM-BINDING**: MIR lower's `collect_pat_bindings_for_mir` was a
   pure "allocate locals" function — it didn't generate any payload
   extraction statements. The fix is a new function
   `lower_enum_variant_pattern_bindings` that generates
   `binding_local = Copy(scrut.Field(field_idx, field_ty))` assignments.

### Approach Chosen (per §15 — 最优 > 最小)

**Flat layout** — flatten ALL non-empty variants' payload fields into
the storage struct. Rejected alternatives:

| Approach | Why rejected |
|----------|-------------|
| **B. Largest payload width** | Only works for same-kind integer payloads. Breaks for mixed types like `enum E { A, B(i32), C(f64) }`. |
| **C. Byte array `[N x i8]`** | Loses type info for codegen optimization. Requires bitcasts for all access. |
| **D. Union struct with per-variant slots (rustc-style)** | Would require nested `Field(Field(Local))` projections. Codegen's `codegen_lvalue_load_typed` doesn't handle nested Field correctly — it loads the intermediate as a value, not a pointer, then tries to GEP into the value. Reworking this would balloon scope. |

Flat layout keeps all projections single-level (`Field(N, ty)` on Local),
matching the existing Case A/B behavior and avoiding codegen rework. The
trade-off is slight storage waste for enums with many variants of
different sizes (each variant's slot is sized to its own payload, not the
max), but this is sound and simple.

### Fix (2 source files)

1. **`src/codegen/mod.rs`** — `mir_type_to_emit_type_with_layouts`:
   - For `AdtLayout::Enum`: flatten ALL variants' payload fields (was:
     only first non-empty). Storage is now:
     - Case A (all unit): `{ discr }` (unchanged)
     - Case B (one non-empty): `{ discr, payload_fields... }` (unchanged)
     - Case C (≥2 non-empty): `{ discr, variant_0_fields..., variant_1_fields..., ... }`
       (NEW — soundness fix; unit variants contribute no fields)
   - For `Tuple/Array/Ref/RawPtr/Slice`: recurse with `_with_layouts`
     (was: fell through to `mir_type_to_emit_type` which doesn't know
     about AdtLayouts). Fixes a pre-existing bug where nested Adts
     (e.g., enum inside a tuple) collapsed to I32. Exposed by the
     e07_enum_in_tuple audit case.

2. **`src/codegen/mod.rs`** — `Rvalue::Aggregate(Adt(...))` codegen:
   - For enum variants: look up `AdtLayout::Enum`, compute the starting
     field_idx (`1 + sum(field_counts of variants 0..V-1)`), insert the
     discriminant at field 0, insert each operand at
     `starting_field_idx + i`. Other variants' slots remain `undef`.

3. **`src/mir/lower/mod.rs`** — new `lower_enum_variant_pattern_bindings`
   function:
   - For `TupleStruct`/`Struct` patterns on enum variants: resolve
     variant_idx + field_tys from HIR (per §16.2.1 — data flows
     downstream), compute the flat field_idx via
     `compute_enum_payload_starting_idx`, generate
     `binding_local = Copy(scrut.Field(field_idx, field_ty))`.
   - Called alongside `collect_pat_bindings_for_mir` in both arm-block
     and otherwise-block lowering paths.

4. **`src/mir/lower/mod.rs`** — new `compute_enum_payload_starting_idx`
   helper:
   - Computes `1 + sum(field_counts of variants 0..V-1)` from HIR.
   - Per §16: reads HIR (allowed — data flows downstream per §16.2.1).

### Resulting IR

```llvm
; enum E { A, B(i32), C(i64) } fn f() -> E { E::C(42) }
define { i32, i32, i64 } @landin_f() {
  ...
  %v1 = insertvalue { i32, i32, i64 } undef, i32 2, 0    ; discriminant = 2
  %v2 = insertvalue { i32, i32, i64 } %v1, i64 42, 2     ; payload at field 2
  store { i32, i32, i64 } %v2, %loc_3
  ...
  ret { i32, i32, i64 } %v4
}

; enum E { A, B(i32), C(i64) } fn f(e: E) -> i64 { match e { E::C(x) => x, _ => 0 } }
  ...
  switch i32 %v4, label %bb2 [
    i32 2, label %bb3
  ]
bb3:
  %v10 = getelementptr inbounds { i32, i32, i64 }, { i32, i32, i64 }* %loc_1, i32 0, i32 2
  %v11 = load i64, %v10                ; extract C's i64 payload from field 2
  store i64 %v11, %loc_5               ; store to binding x's local
  %v12 = load i64, %loc_5              ; arm body reads x — actual payload, not uninit
  store i64 %v12, %loc_2
  br label %bb1
```

### §15.4 Verification (root-cause fix confirmed)

Per §15.4.4, the gate review must verify the root cause is actually
fixed. Three verification points:

1. **L-ENUM-UNION layout**: `e01_case_c_layout` and `e05_enum_param_case_c`
   audit cases verify `{ i32, i32, i64 }` is produced for Case C enums.
   The old buggy `{ i32, i32 }` is explicitly asserted as `expect_none`
   in `u01_case_c_layout`.

2. **L-ENUM-UNION construction**: `u02_case_c_variant_c_ctor` verifies
   `E::C(42)` inserts `i32 2, 0` (discr) then `i64 42, 2` (payload at
   field 2). The payload no longer overflows into B's i32 slot.

3. **L-ENUM-BINDING**: `u05_binding_case_b`, `u06_binding_case_c_b`,
   `u07_binding_case_c_c` verify that match arms with tuple-variant
   patterns generate `getelementptr` to the correct field index. The
   binding local is now populated from the enum's payload, not left
   uninitialized.

---

## 4. Committee Vote: 5/5 APPROVED — UNANIMOUS

---

## 5. Updated Limitation List

| ID | Status |
|----|--------|
| L-ENUM-UNION (enum union payload) | **CLOSED in Stage 3.48** ✅ |
| L-ENUM-BINDING (pattern binding extraction — P0 hidden) | **CLOSED in Stage 3.48** ✅ |
| Pre-existing bug: nested Adts in Tuple/Array/Ref collapsed to I32 | **CLOSED in Stage 3.48** ✅ |
| All prior CLOSED items (L2/L4/L6/L7/L9/L10/L11/L12/L14/L15/L-DEBT-2/L-MUT-1/L-DEBT-3/L-ENUM/L-ENUM-MATCH/L-CONST/L-PIPE-1) | ✅ |
| Remaining open: L1 (PHI), L3 (closures), L5 (traits), L8 (lli), L13 (fat ptr), L-COPY-ADT | Open |

---

## 6. Test Progression

| Version | Tests | Delta |
|---------|-------|-------|
| v0.8.6 (3.45, L10 float bitwise) | 842 | +6 |
| v0.8.6 (3.46, L14 + L9 integer types) | 855 | +13 |
| v0.8.6 (3.47, L-PIPE-1 closure) | 869 | +14 |
| **v0.8.6 (3.48, L-ENUM-UNION + L-ENUM-BINDING)** | **881** | **+12** |

### New tests added in Stage 3.48 (12)

| Test | Asserts |
|------|---------|
| `codegen_enum_union_two_payloads_layout` | Case C layout `{ i32, i32, i64 }` |
| `codegen_enum_union_variant_c_construction` | `E::C(42)` inserts discr=2, i64 at field 2 |
| `codegen_enum_union_variant_b_construction` | `E::B(7)` inserts discr=1, i32 at field 1 |
| `codegen_enum_union_match_b_extracts_payload` | `E::B(x) => x` GEPs to field 1 |
| `codegen_enum_union_match_c_extracts_payload` | `E::C(x) => x` GEPs to field 2 |
| `codegen_enum_binding_extraction_case_b` | `Opt::Some(x) => x` extracts payload (P0 fix) |
| `codegen_enum_union_multi_field_variant_layout` | `B(i32, i64)` → `{ i32, i32, i64, i64 }` |
| `codegen_enum_union_mixed_types_layout` | `B(i32), C(f64)` → `{ i32, i32, double }` |
| `codegen_enum_union_regression_single_payload` | Case B unchanged: `{ i32, i32 }` |
| `codegen_enum_union_regression_all_unit` | Case A unchanged: `{ i32 }` |
| `codegen_enum_union_struct_variant_match` | `Point { x, y }` extracts both fields |
| `codegen_enum_union_match_returns_correct_value` | End-to-end: arm returns payload, not 0 |

---

## 7. Audit Coverage Cross-check (per §17)

| Audit dimension | Cases | Source |
|-----------------|-------|--------|
| Case C layout/ctor | u01-u04 (4) | new in R15 |
| L-ENUM-BINDING (Case B + C) | u05-u07 (3) | new in R15 |
| Multi-field + mixed types | u08-u10 (3) | new in R15 |
| Struct variant binding | u11 (1) | new in R15 |
| Case A/B regression | u12-u14 (3) | new in R15 |
| Edge: 3-variant/bool/struct/return/param/wildcard/tuple/two-enums | e01-e08 (8) | new in R15 |
| Regression from R14 | r01-r08 (8) | carried forward |
| **Total** | **30** | ✅ ≥30 per §9.3.1 |

---

## 8. §18 Document Sync Compliance (process v3.13)

Per §18.3, the following documents have been updated as part of this round:

| Document | Status |
|----------|--------|
| `docs/develop/v0/stage-3/dev-log.md` | ✅ Stage 3.48 entry added |
| `docs/develop/v0/stage-3/gate-review-round15.md` | ✅ This file |
| `docs/tests/matrix.md` | ✅ Updated (881 tests, L-ENUM-UNION + L-ENUM-BINDING CLOSED) |
| `docs/tests/v0/stage3/plan/codegen_enum.md` | ✅ Updated (Stage 3.48 flat layout + binding extraction) |
| `docs/lang-design/06-mir.md` | ✅ Updated (Stage 3.48 flat enum layout) |
| `README.md` | ✅ Updated (881 tests, 15 rounds) |
| `examples/stage3_gate_audit_r15.rs` | ✅ Created (30 cases) |
| `examples/stage3_gate_audit_r14.rs` | ✅ Updated (i14 case asserts new correct layout) |
| `worklog.md` | ✅ Stage 3.48 entry to be appended |

---

## 9. Conclusion

Stage 3 Round 15 **PASSED** with unanimous 5/5 approval. All 30 audit
cases pass, all 881 tests pass, 0 clippy warnings, 0 fmt issues.

**L-ENUM-UNION CLOSED**. Enum storage layout now flattens ALL non-empty
variants' payload fields. The silent memory corruption bug (writing an
i64 payload into an i32 slot) is fixed.

**L-ENUM-BINDING CLOSED**. Pattern matching on enum tuple/struct variants
now generates payload-extraction projections. The hidden P0 bug (reading
uninitialized memory for bindings) is fixed.

**Bonus fix**: pre-existing bug where nested Adts inside Tuple/Array/Ref
collapsed to I32 is also closed (was exposed by the e07_enum_in_tuple
audit case).

**Remaining open limitations**: L1 (PHI optimization), L3 (closures),
L5 (trait dispatch), L8 (lli execution), L13 (fat pointers), L-COPY-ADT.
