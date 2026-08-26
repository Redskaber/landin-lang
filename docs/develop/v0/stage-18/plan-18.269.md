# Stage 18.269 — Phase 2d Implementation + Continued Audit

> **Author**: Super Z (main) — Stage Committee (ARCH-A + DEV-A + QA-A)
> **Date**: 2026-08-25
> **Version**: v0.492.0 (no bump — soundness fix)
> **Process**: stage-committee-process.md v6.4 §17.6 (缺陷纳入 — "直到审查不出问题为止")
> **Status**: ✅ Complete — Phase 2d implemented, deeper issue documented

---

## 1. Executive Summary

This stage implements Phase 2d of TD-GENERIC-FN-RETURN-EXPECTED-TY
(threading `expected_ty = return_mir_ty` into fn body tail expression).
The fix is additive but reveals a deeper issue: `return_mir_ty` for
`Holder<i32>` is lowered as `Adt(holder_def, [])` (empty substs) because
the HIR return type path resolution doesn't extract substs in fn sig
context.

### 1.1 Outcomes

| Aspect | Value |
|--------|-------|
| Phase 2d implemented | ✅ `expected_ty = return_mir_ty` threaded |
| New soundness holes closed | 0 (Phase 2d alone doesn't close the gap due to deeper path resolution issue) |
| New MVP documented | TD-RETURN-TY-PATH-SUBSTS (deeper path resolution issue) |
| Test count | 3895 (unchanged), 0 failures |
| Files modified | 1 (`mir/lower/body_lower.rs`) |

### 1.2 Verification

- ✅ `cargo build --features llvm-backend` — 0 warnings
- ✅ `cargo check --features llvm-backend` — 0 errors, 0 warnings
- ✅ `cargo fmt --check` — 0 diff
- ✅ `cargo clippy --all-targets --features llvm-backend -- -D warnings` — 0 warnings
- ✅ `cargo test --features llvm-backend` — 3895 tests, 0 failures

---

## 2. Implementation

### 2.1 Phase 2d: Thread return_mir_ty into body tail expression

In `src/mir/lower/body_lower.rs` line ~411:

```rust
let return_is_unit_for_expected =
    matches!(&return_mir_ty.kind, TyKind::Tuple(tys) if tys.is_empty());
let return_ty_for_expected: Option<Ty> = if return_is_unit_for_expected {
    None  // Don't thread for void fns (unit unifies with anything)
} else {
    Some(return_mir_ty.clone())
};
let value_local =
    lower_expr_to_operand(&mut cx, &body.value, return_ty_for_expected.as_ref());
```

Also cloned `return_mir_ty` before passing to `new_local_with_mut`
(line 310) to allow reuse.

### 2.2 Deeper Issue Discovered

When testing `fn make_holder() -> Holder<i32> { Holder(true) }`,
the fix doesn't close the soundness hole because `return_mir_ty`
for `Holder<i32>` is `Adt(holder_def, [])` (empty substs).

Root cause: `lower_hir_ty_to_mir_ty_with_lifetimes` (used for fn sig
return types) calls `lower_path_generic_args` which should extract
substs from `Holder<i32>`. But debug shows substs are empty.

This is a separate path resolution issue — when the path is used in
a fn sig context (`-> Holder<i32>`), the generic args `<i32>` may
not be parsed/lowered correctly.

Per §17.6 "直到审查不出问题为止": this needs continued investigation.
Documented as new TD-RETURN-TY-PATH-SUBSTS (MVP, deferred to deeper
audit).

---

## 3. §13.4 J1-J6 Audit

| # | Judgment | Verification |
|---|----------|-------------|
| J1 | Architecture alignment | ✅ Aligns with Phase 2b/2c pattern |
| J2 | Single responsibility | ✅ expected_ty threading encapsulated |
| J3 | One-way flow | ✅ return_ty → expected_ty → body tail |
| J4 | Compile-concept completeness | ✅ Same expected_ty concept |
| J5 | Stage division | ✅ Only touches mir/lower/body_lower.rs |
| J6 | Reasonable size | ✅ ~15 LOC change |

**All 6 judgments pass.**

---

## 4. Committee Voting

| Role | Vote | Reason |
|------|------|--------|
| ARCH-A | APPROVED | Phase 2d is correct (additive); deeper issue documented |
| DEV-A | APPROVED | ~15 LOC change; low risk |
| QA-A | APPROVED | No regression; 3895 tests pass |

**Result: 3/3 APPROVED**

---

## 5. Action Plan

| # | Action | Stage | Owner | Status |
|---|--------|-------|-------|--------|
| 1 | Implement Phase 2d (thread return_mir_ty) | 18.269 | DEV-A | ✅ Done |
| 2 | Document TD-RETURN-TY-PATH-SUBSTS (deeper issue) | 18.269 | REC-A | ✅ Done |
| 3 | Continue §17.6 audit — investigate path resolution for fn sig return types | 18.270+ | ARCH-A | 🔧 Next |

---

## 6. References

- Stage 18.268 plan: `docs/develop/v0/stage-18/plan-18.268.md`
- Tech-debt-register: `docs/develop/v0/tech-debt-register.md` (TD-GENERIC-FN-RETURN-EXPECTED-TY PARTIAL)
