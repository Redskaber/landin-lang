# Stage 15.12 — Error System Cleanup + Friendly Display

> **Author**: redskaber
> **Date**: 2026-07-31
> **Version**: v0.137.0 → v0.138.0
> **Process**: stage-committee-process.md v3.23 §29 (stage-end deep review)

## 1. Executive Summary

Stage 15.12 improves the error system in two ways:

1. **Architectural cleanup**: Removed `MirBody.lower_type_errors` field.
   Type errors collected during MIR lowering are now returned from the
   lowering function as a separate `Vec<TypeError>` in the return tuple.
   This separates IR data from error collection (was an architectural
   smell — IR carrying error collection).

2. **Friendly display**: Improved `format_for_user` to use friendlier
   summary ("error: N errors found" vs "error: N error(s)") and display
   `ResolveError` via `.message` + snippet (was Debug `{:?}`).

Both changes address the user's "留意错误系统、显示友好" (mind the error
system, display friendly) requirement.

## 2. Why This Change?

### 2.1 Architectural smell: IR carrying error collection

Per Phase 2 audit:
- `MirBody.lower_type_errors: Vec<TypeError>` mixed IR data with error
  collection — a violation of separation of concerns.
- The `MirLowerCtxt` already had a `type_errors` field that was unused.
- Errors were pushed to `cx.mir.lower_type_errors` (the IR struct) instead
  of `cx.type_errors` (the context struct).

Per §1.0 原则 3 "显式 > 隐式": errors should be explicit in the function
signature, not implicit on the IR struct.

### 2.2 Unfriendly error display

Per user requirement "显示友好":
- "error: N error(s)" is awkward — "error(s)" with parenthetical is ugly.
- `ResolveError` was displayed via Debug `{:?}` — users see
  `ResolveError { message: "...", span: Span { ... } }` instead of the
  actual message.
- No singular/plural distinction ("1 error" vs "N errors").

## 3. Changes Made

### 3.1 Removed `MirBody.lower_type_errors` field

**Before** (`src/mir/body.rs`):
```rust
pub struct MirBody {
    pub basic_blocks: Vec<BasicBlock>,
    pub local_decls: Vec<LocalDecl>,
    pub span: Span,
    pub adt_layouts: SharedAdtLayouts,
    pub dyn_trait_calls: Vec<DynTraitMethodCall>,
    pub lower_type_errors: Vec<crate::typeck::TypeError>,  // ← REMOVED
}
```

**After**: Field removed. Errors are now returned from the lowering function.

### 3.2 Lowering functions return 3-tuple

**Before** (`src/mir/lower/mod.rs`):
```rust
pub fn lower_hir_body_to_mir_full_with_dyn_trait_plan(...) -> (MirBody, UnificationTable) {
    ...
    (cx.mir, unify)
}
```

**After**:
```rust
pub fn lower_hir_body_to_mir_full_with_dyn_trait_plan(
    ...
) -> (MirBody, UnificationTable, Vec<crate::typeck::TypeError>) {
    ...
    let type_errors = std::mem::take(&mut cx.type_errors);
    (cx.mir, unify, type_errors)
}
```

All 4 lowering entry points updated:
- `lower_hir_body_to_mir` — returns `MirBody` (convenience wrapper, discards unify + errors)
- `lower_hir_body_to_mir_with_return_ty` — returns `MirBody` (convenience wrapper)
- `lower_hir_body_to_mir_full` — returns 3-tuple
- `lower_hir_body_to_mir_full_with_dyn_trait_plan` — returns 3-tuple
- `lower_body_full` — returns 3-tuple (convenience alias)

### 3.3 Updated 8 callsites in expr_operand.rs

All `cx.mir.lower_type_errors.push(...)` → `cx.type_errors.push(...)`.
The `cx.type_errors` field already existed but was unused — now it's the
canonical location for lowering errors.

### 3.4 Updated driver to receive errors from return tuple

**Before** (`src/driver.rs`):
```rust
let (mut mir, lower_unify) = lower_hir_body_to_mir_full_with_dyn_trait_plan(...);
errors.typeck.append(&mut mir.lower_type_errors);
```

**After**:
```rust
let (mut mir, lower_unify, lower_type_errors) =
    lower_hir_body_to_mir_full_with_dyn_trait_plan(...);
errors.typeck.extend(lower_type_errors);
```

### 3.5 Friendly error display

**Summary line** (`src/driver.rs` `format_for_user`):
- Before: `error: 3 error(s)\n`
- After: `error: 3 errors found\n` (or `error: 1 error found` for singular)

**ResolveError display**:
- Before: `[resolve] ResolveError { message: "...", span: Span { ... } }\n`
- After: `[resolve] <message>\n` + snippet (same as typeck/borrowck)

## 4. §29 Stage-End Deep Review

### 4.1 Data flow coverage (§29.1.1)

Error data flow is now cleaner:
- Before: `cx.mir.lower_type_errors.push(...)` → `mir.lower_type_errors` → driver drains
- After: `cx.type_errors.push(...)` → returned from lower fn → driver extends

The errors are no longer stored on the IR — they flow through the function
return value, which is the standard Rust pattern.

### 4.2 Architecture review (§29.1.2)

**Structural clarity** ✅ — `MirBody` is now pure IR data (no error collection).
Errors are returned from the lowering function, separating concerns.

**Efficiency** ✅ — no change (same Vec, just stored in a different place).

**Extensibility** ✅ — adding new error types to the lowering function is
now explicit in the return type, not hidden in the IR struct.

### 4.3 Design-impl-test coverage (§29.1.3)

| Design point | Implementation | Test |
|--------------|----------------|------|
| `lower_type_errors` removed from MirBody | Field deleted | `stage15_12_mirbody_no_lower_type_errors_field` |
| Lower fn returns 3-tuple | Return type changed | (implicit — all tests compile) |
| Errors flow through return value | `cx.type_errors` → return | (implicit — tests pass) |
| Friendly summary "errors found" | `format_for_user` updated | `stage15_12_error_summary_friendly_format` |
| Singular "1 error found" | `format_for_user` updated | `stage15_12_singular_error_count` |
| ResolveError displays via .message | `format_for_user` updated | `stage15_12_resolve_error_display` |
| No errors → empty string | `format_for_user` returns "" | `stage15_12_no_errors_empty_output` |

### 4.4 Hidden problems (§29.1.4)

| Hidden problem | Complexity growth | Stage 15.12 status |
|----------------|-------------------|-------------------|
| `ResolveError` has no error code (E001) | 1× (future work) | Deferred — not blocking |
| No color output (ANSI codes) | 1× (LSP mode) | Deferred — CLI only for now |
| No "help:" suggestions | 1× (future work) | Deferred — requires error catalog |

No new hidden problems. The error system is now cleaner architecturally.

### 4.5 Refactoring optimality (§29.2)

**Approach taken** ✅ — Returning errors from the function is the standard
Rust pattern (cf. `io::Result`, `fmt::Result`). The `cx.type_errors` field
already existed — we just use it now.

**Alternative considered** ✅ — Could have kept `lower_type_errors` on
MirBody and just fixed the display. Rejected because the architectural
smell (IR carrying error collection) would remain.

**Skipped refactors** ✅ — Did not add error codes (E001, E002, etc.) or
color output. Those are larger features that need their own stage.

## 5. Test Results

| Test suite | Before (v0.137.0) | After (v0.138.0) | Delta |
|------------|-------------------|------------------|-------|
| Rust unit (lib) | 145 | 145 | 0 |
| Rust integration (all_tests) | 1990 | 1998 | +8 (error system tests) |
| Conformance (.lin) | 5216 | 5216 | 0 |
| **Total** | **7351** | **7359** | **+8** |

All tests pass. Zero regressions. Zero clippy warnings. fmt clean.

## 6. Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Version bump 0.137.0 → 0.138.0 |
| `src/mir/body.rs` | Removed `lower_type_errors` field from `MirBody` |
| `src/mir/lower/mod.rs` | 4 lowering functions return 3-tuple `(MirBody, UnificationTable, Vec<TypeError>)`; `lower_body_full` updated |
| `src/mir/lower/expr_operand.rs` | 8 `cx.mir.lower_type_errors.push` → `cx.type_errors.push` |
| `src/driver.rs` | Updated to receive errors from return tuple; friendly summary line; ResolveError displays via .message |
| `tests/v0/stage2/plan/typeck_tests.rs` | Updated destructuring to 3-tuple |
| `tests/v0/stage5/plan/driver_dyn_trait_plan_integration_tests.rs` | Updated destructuring to 3-tuple (6 sites) |
| `tests/v0/stage5/plan/mir_lower_dyn_trait_method_call_integration_tests.rs` | Updated destructuring to 3-tuple |
| `tests/v0/stage2/plan/integration_tests.rs` | Updated assertion for new "errors found" format |
| `tests/v0/stage15/plan/error_system_cleanup_tests.rs` | **NEW** — 8 integration tests |
| `tests/all_tests.rs` | Registered `stage15_error_system_cleanup_tests` |
| `docs/develop/v0/stage-15/stage-15.12-error-system-cleanup.md` | This document |
| `docs/tests/v0/stage15/stage-15.12-test-plan.md` | **NEW** — test plan |
| `docs/worklog.md` | Stage 15.12 entry appended |
| `RELEASE_NOTES.md` | v0.138.0 entry appended |
| `README.md` | Updated with Stage 15.12 progress |
