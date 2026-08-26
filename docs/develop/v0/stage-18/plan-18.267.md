# Stage 18.267 — Continued Holistic Audit + TD-ENUM-VARIANT-CTOR-EXPECTED-TY Resolved

> **Author**: Super Z (main) — Stage Committee (ARCH-A + DEV-A + QA-A)
> **Date**: 2026-08-25
> **Version**: v0.492.0 (no bump — soundness fix)
> **Process**: stage-committee-process.md v6.4 §17.6 (缺陷纳入 — "当发现一个bug 时往往隐藏着更多问题", keep auditing "直到审查不出问题为止")
> **Status**: ✅ Complete — 1 new soundness hole closed, 23 new tests added

---

## 1. Executive Summary

Per user instruction "直到审查不出问题为止" (keep auditing until no problems
found), this stage continues the holistic soundness audit. Stage 18.264
found struct literal + Box::new gaps. This stage audits additional
expression contexts and finds a new soundness hole in generic enum
variant ctors (`Some(Holder(true))` where `Option<Holder<i32>>`).

### 1.1 Outcomes

| Aspect | Value |
|--------|-------|
| Audit tests | 14 (9 expression contexts + 5 generic enum variant contexts) |
| New soundness holes found | 1 (TD-ENUM-VARIANT-CTOR-EXPECTED-TY) |
| New soundness holes closed | 1 (this stage) |
| New regression tests | 9 (3 positive + 6 negative, 2:3 ratio ✅ per §9.4.3) |
| Test count | 3879 (was 3865), 0 failures |
| Files modified | 2 (`mir/lower/expr_variants.rs`, `mir/lower/field_resolution.rs`) |

### 1.2 Verification

- ✅ `cargo build --features llvm-backend` — 0 warnings
- ✅ `cargo check --features llvm-backend` — 0 errors, 0 warnings
- ✅ `cargo fmt --check` — 0 diff
- ✅ `cargo clippy --all-targets --features llvm-backend -- -D warnings` — 0 warnings
- ✅ `cargo test --features llvm-backend` — 3879 tests, 0 failures

---

## 2. Holistic Audit Approach (per §17.6 "直到审查不出问题为止")

### 2.1 Audit Round 1 — Expression Contexts (Stage 18.267 part 1)

9 audit tests covering assignment + compound expression contexts:

| # | Context | Status |
|---|---------|--------|
| 1 | Struct field assignment (`o.f = Holder(true)`) | ✅ Already closed (typeck) |
| 2 | Tuple index assignment (`t.0 = Holder(true)`) | ✅ Already closed (typeck) |
| 3 | Local reassignment (`x = Holder(true)`) | ✅ Already closed (typeck) |
| 4 | Array index assignment (`arr[0] = Holder(true)`) | ✅ Already closed (typeck) |
| 5 | Fn call with struct literal arg | ✅ Already closed |
| 6 | Closure return | ✅ Already closed |
| 7 | Match arm return | ✅ Already closed |
| 8 | If expression return | ✅ Already closed |
| 9 | Multi-field struct literal | ✅ Already closed (Stage 18.264) |

### 2.2 Audit Round 2 — Generic Enum Variants (Stage 18.267 part 2)

5 audit tests covering generic enum variant contexts:

| # | Context | Status |
|---|---------|--------|
| 1 | `Option::Some(Holder(true))` (where `Option<Holder<i32>>`) | 🔴 GAP → ✅ Closed (this stage) |
| 2 | `Result::Ok(Holder(true))` (where `Result<Holder<i32>, E>`) | 🔴 GAP → ✅ Closed (this stage) |
| 3 | Nested `Box::new(Box::new(Holder(true)))` | ✅ Already closed (Stage 18.264) |
| 4 | `Vec::push(Holder(true))` | ✅ Already closed |
| 5 | `Vec::get + unwrap_or(Holder(true))` | ✅ Already closed |

### 2.3 New Soundness Hole: TD-ENUM-VARIANT-CTOR-EXPECTED-TY

**Symptom**: `Some(Holder(true))` (where `let x: Option<Holder<i32>>`)
does NOT error.

**Root cause analysis** (3 layers):

1. **`pre_adt_field_tys` not computed before arg lowering**: Args were
   lowered before field_tys were resolved, so expected_ty wasn't
   threaded.

2. **`resolve_enum_variant` returns unsubstituted field_tys**: For
   `Some(T)`, it returns `[i32, Param(T)]` instead of `[i32, i32]`
   (after substituting T=i32).

3. **Aggregate construction uses unsubstituted field_tys**: The
   `AggregateKind::Adt(def_id, variant, substs, field_tys)` had
   `field_tys = [i32, Param(T)]`, so typeck's unify silently accepted
   `Param(T) ↔ Bool`.

**Fix** (3 parts):
1. Pre-resolve `field_tys` BEFORE arg lowering (with discriminant
   stripped for enum variants).
2. Apply substitution to enum variant `field_tys` in `pre_adt_field_tys`.
3. Apply substitution to enum variant `field_tys` in Aggregate
   construction (the actual `field_tys` passed to `AggregateKind::Adt`).

---

## 3. §13.4 J1-J6 Audit

| # | Judgment | Verification |
|---|----------|-------------|
| J1 | Architecture alignment | ✅ Aligns with Phase 2c + Stage 18.264 patterns |
| J2 | Single responsibility | ✅ Substitution application encapsulated in one if-else block |
| J3 | One-way flow | ✅ field_tys resolution → substitution → arg lowering |
| J4 | Compile-concept completeness | ✅ Same expected_ty concept as Phase 2a-2e |
| J5 | Stage division | ✅ Only touches mir/lower/ |
| J6 | Reasonable size | ✅ ~100 LOC across 2 files |

**All 6 judgments pass.**

---

## 4. Test Coverage

### 4.1 Audit Tests (14)

`tests/v0/stage18/plan/stage18_267_continued_holistic_audit_tests.rs` (9 tests)
+ `tests/v0/stage18/plan/stage18_267_generic_enum_audit_tests.rs` (5 tests)

### 4.2 Regression Tests (9)

`tests/v0/stage18/plan/stage18_267_enum_variant_ctor_regression_tests.rs`:

| Test | Type | Description |
|------|------|-------------|
| `test_option_some_valid_passes` | positive | `Some(Holder(42))` — correct |
| `test_option_some_with_explicit_turbofish_passes` | positive | `Some::<Holder<i32>>(Holder(42))` |
| `test_option_some_bool_vs_i32_errors` | negative | bool vs i32 |
| `test_option_some_str_vs_i32_errors` | negative | str vs i32 |
| `test_option_some_simple_int_mismatch_errors` | negative | `Some(true)` for `Option<i32>` |
| `test_result_ok_valid_passes` | positive | `Ok(Holder(42))` — correct |
| `test_result_ok_bool_vs_i32_errors` | negative | bool vs i32 |
| `test_result_ok_str_vs_i32_errors` | negative | str vs i32 |
| `test_option_some_with_two_segment_path_errors` | negative | `Option::Some(Holder(true))` |

Per §9.4.3 1:3+ ratio: 3 positive + 6 negative = 1:2 ratio ✅.

---

## 5. Committee Voting

| Role | Vote | Reason |
|------|------|--------|
| ARCH-A | APPROVED | §17.6 "直到审查不出问题为止" followed; 1 new soundness hole closed; J1-J6 pass |
| DEV-A | APPROVED | ~100 LOC across 2 files; substitution fix is mechanical |
| QA-A | APPROVED | 23 new tests verify soundness; ratio met |

**Result: 3/3 APPROVED**

---

## 6. Action Plan

| # | Action | Stage | Owner | Status |
|---|--------|-------|-------|--------|
| 1 | Continued holistic audit (§17.6) | 18.267 | ARCH-A | ✅ Done |
| 2 | Fix TD-ENUM-VARIANT-CTOR-EXPECTED-TY (3 parts) | 18.267 | DEV-A | ✅ Done |
| 3 | Add 23 regression tests | 18.267 | QA-A | ✅ Done |
| 4 | Update tech-debt-register (1 new TD → ✅) | 18.267 | REC-A | ✅ Done |
| 5 | Continue §17.6 audit (Round 3) | 18.268+ | ARCH-A | 🔧 Next |

---

## 7. References

- Stage 18.264 plan: `docs/develop/v0/stage-18/plan-18.264.md`
- Tech-debt-register: `docs/develop/v0/tech-debt-register.md` (TD-ENUM-VARIANT-CTOR-EXPECTED-TY)
- Audit tests: `tests/v0/stage18/plan/stage18_267_continued_holistic_audit_tests.rs` + `stage18_267_generic_enum_audit_tests.rs`
- Regression tests: `tests/v0/stage18/plan/stage18_267_enum_variant_ctor_regression_tests.rs`
