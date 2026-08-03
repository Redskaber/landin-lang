# Stage 16.05 — Field-not-found Error Reporting (Last TODO Resolution)

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.226.3 → v0.226.4
> **Process**: stage-committee-process.md v3.24 §1.0 原則 4 "报错 > 静默" + §23 API 命名标准化

## 1. Executive Summary

Stage 16.05 resolves the **last remaining TODO** from the Stage 16.00 v0.3
kickoff audit. The TODO was in `src/mir/lower/field_resolution.rs:86` —
when a field access (`s.y`) targets a field that doesn't exist on the
receiver's struct, the error was silently dropped because
`resolve_field_index` took `cx: &MirLowerCtxt` (immutable) and could not
push to `cx.type_errors`.

**Fix**: Changed `resolve_field_index` to take `cx: &mut MirLowerCtxt`,
enabling direct error reporting. The error message is
`no field \`{name}\` on struct \`{struct}\``, and the span points to the
receiver expression. The fallback return value of 0 is preserved for
codegen recovery (the error will abort compilation before codegen runs).

**Result**: v0.3 cleanup complete — **0 TODOs remaining in `src/`**.
+6 integration tests, +1 conformance test flipped (limitation removed).

## 2. Background

### 2.1 The TODO

The TODO was documented in the Stage 16.00 v0.3 kickoff as one of 3
remaining low-priority items:

| # | Location | Priority | Resolved At |
|---|----------|----------|-------------|
| 1 | `borrowck/mod.rs:246` Span::DUMMY (region error span) | Low | Stage 16.04 ✅ |
| 2 | `mir/lower/field_resolution.rs:86` (MirLowerCtxt mutability) | Low | **Stage 16.05** ✅ |
| 3 | `mir/lower/mod.rs:1317` (explicit lifetime name tracking) | Low | Stage 16.01 ✅ |

### 2.2 The Problem

`resolve_field_index` is called when lowering a field-access expression
`receiver.ident`. It looks up the field index in the HIR struct
definition. When the field is not found, it returned 0 as a fallback
(silent wrong behavior):

```rust
// Stage 14.31: Per "报错 > 静默" — field not found in the
// receiver's struct. Emit error instead of silently returning 0.
// Note: cx is &MirLowerCtxt (immutable), so we can't push to
// lower_type_errors here. Instead, we'll rely on typeck to
// catch this — the field_ty resolution returns None, which
// means the result local gets fresh_infer_ty (Infer), and
// typeck will report a type error if the field is used in a
// context that expects a specific type.
//
// TODO: When MirLowerCtxt is made mutable in this function,
// push the error directly. For now, the fallback behavior
// (return 0) is preserved but the field access will produce
// wrong results — typeck should catch it in most cases.
return 0; // fallback for codegen (error will abort before codegen)
```

The comment explicitly says "typeck should catch it in most cases" —
but "in most cases" is not "always". The silent fallback violated
§1.0 原則 4 "报错 > 静默" (error > silence).

### 2.3 Why It Wasn't Fixed Earlier

The function signature was `cx: &MirLowerCtxt` (immutable) because
`resolve_field_index` also calls `find_receiver_struct_def_id(cx, ...)`
which takes `&MirLowerCtxt`. Changing to `&mut` would require:
1. Updating the function signature.
2. Verifying all callers have `&mut` access.
3. Ensuring no borrow conflicts (the function reads `cx.hir` and
   `cx.interner` while also wanting to push to `cx.type_errors`).

The callers (`lower_expr_to_place` and `lower_expr_to_operand`) both
take `cx: &mut MirLowerCtxt`, so the change is clean. The borrow
conflict doesn't arise because `cx.hir` is `Option<&'a HirCrate>` (the
reference outlives the borrow of `cx`), and `cx.interner` is `&'a Rodeo`
(same).

## 3. Implementation

### 3.1 Signature Change

```rust
// Before:
pub(crate) fn resolve_field_index(
    cx: &MirLowerCtxt,
    receiver: &HirExpr,
    field_name: &crate::lexer::Symbol,
) -> u32 {

// After:
pub(crate) fn resolve_field_index(
    cx: &mut MirLowerCtxt,
    receiver: &HirExpr,
    field_name: &crate::lexer::Symbol,
) -> u32 {
```

Per §23.1 rule 7 (function naming prefix): `resolve_` prefix is correct
for name resolution helpers. No naming change needed.

### 3.2 Error Reporting

When the field is not found in the receiver's struct, the code now
pushes a `TypeError` directly:

```rust
let struct_name = cx
    .interner
    .try_resolve(&s.ident.name)
    .unwrap_or("<anonymous>");
let field_name_str = cx
    .interner
    .try_resolve(field_name)
    .unwrap_or("<unknown>");
cx.type_errors.push(crate::typeck::TypeError::new(
    format!(
        "no field `{}` on struct `{}`",
        field_name_str, struct_name
    ),
    receiver.span,
));
return 0; // fallback for codegen (error will abort before codegen)
```

Per §1.0 原則 3 "显式 > 隐式": the error message uses human-readable
names (resolved from the interner), not Debug `{:?}` output.

Per §1.0 原則 4 "报错 > 静默": the error is reported, not silently
dropped. The fallback return value of 0 is preserved so codegen can
still proceed (the error will abort compilation before codegen runs,
but having a valid return value prevents cascading panics).

### 3.3 Span Accuracy

The error span is `receiver.span` — the span of the receiver expression
(`s` in `s.y`). This points the user to the expression that has the
wrong field access, not to the struct definition or some other location.

### 3.4 Caller Updates

Both callers already had `cx: &mut MirLowerCtxt`:
- `lower_expr_to_place(cx: &mut MirLowerCtxt, ...)` in `expr_operand.rs:64`
- `lower_expr_to_operand(cx: &mut MirLowerCtxt, ...)` in `expr_operand.rs:377`

No caller changes needed — Rust auto-reborrows `&mut T` to `&mut T`.

## 4. Conformance Test Update

The conformance test `tests/conformance/01-typecheck/99-error-cases/026-undefined-struct-field.lin`
was previously marked `EXPECTED: compile_ok` (Stage 0 limitation —
typeck did not catch this). With Stage 16.05, the error is now caught,
so the test is flipped to `EXPECTED: compile_error` with
`ERROR_PATTERN: no field`.

This removes a known limitation and moves the test from "documented
limitation" to "enforced correctness".

## 5. Integration Tests

Added `tests/v0/stage15/plan/stage16_05_field_not_found_error_tests.rs`
with 6 tests:

1. `stage16_05_undefined_field_reports_error` — error is produced
2. `stage16_05_error_message_contains_field_name` — message has field name
3. `stage16_05_error_message_contains_struct_name` — message has struct name
4. `stage16_05_error_span_points_to_receiver` — span is not DUMMY, points to receiver
5. `stage16_05_valid_field_access_no_error` — regression: valid access doesn't error
6. `stage16_05_multiple_undefined_fields_each_error` — non-fatal: multiple errors

Per §29.1.3 (Design-Impl-Test coverage): tests verify both the error is
reported AND the error message/span is correct.

## 6. API Naming Standard Compliance (§23)

| Rule | Compliance |
|------|------------|
| §23.1.1 Free function entry | ✅ `resolve_field_index` is a free function |
| §23.1.2 Context type naming | ✅ `MirLowerCtxt` follows `<Stage>LowerCtxt<'a>` pattern |
| §23.1.3 Type prefix | ✅ `MirLowerCtxt` has `Mir` prefix |
| §23.1.4 Re-export style | N/A (no re-export changes) |
| §23.1.5 DRY | ✅ No duplicate definitions |
| §23.1.6 Deprecation | N/A (no deprecated items) |
| §23.1.7 Function naming prefix | ✅ `resolve_` prefix for name resolution |
| §23.1.8 Error type suffix | ✅ `TypeError` has `Error` suffix |

## 7. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2150/2150 PASS (+6 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7618 tests passing, 0 failures, 0 warnings.**
- **0 TODOs remaining in `src/`** (down from 3 at Stage 16.00)

## 8. Version Policy

v0.226.3 → v0.226.4 (patch bump — TODO resolution, error reporting
improvement. No behavior change for valid programs; new error for
previously-silent invalid field access.)

## 9. v0.3 Roadmap Status

| Item | Status |
|------|--------|
| TODO cleanup (3 items) | ✅ COMPLETE (Stages 16.01, 16.04, 16.05) |
| Sound Copy migration (117 tests) | 🔧 Pending (53% complete) |
| Task 3: TraitResolver Keys | 🔧 Pending (2 weeks) |
| Task 11: Monomorphization | 🔧 Pending (2-3 weeks) |
| Task 10: Closure redesign | 🔧 Pending (2-3 weeks) |

**Next step**: Sound Copy migration (manual review of remaining 117 tests).
