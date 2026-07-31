# Stage 15.9 — Test Plan

> **Date**: 2026-07-31
> **Version**: v0.134.0 → v0.135.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3

## 1. Test Scope

Stage 15.9 makes two type changes:
1. `VtableEntry.fn_name: String` → `Spur` (interned)
2. `CompileErrors.trait_errors: Vec<String>` → `Vec<TraitError>`

| Area | Test type | Count |
|------|-----------|-------|
| VtableEntry.fn_name interning | Integration | 2 new |
| TraitError structured data | Integration | 2 new |
| TraitError::format_with_interner | Integration | 1 new |
| format_for_user with interner | Integration | 1 new |
| Regression (existing tests) | All existing | 1970 + 5216 |

## 2. Integration Test Module

**Path**: `tests/v0/stage15/plan/vtable_interning_and_trait_error_tests.rs`
**Registered as**: `stage15_vtable_interning_and_trait_error_tests`

### 2.1 Test cases

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_9_vtable_fn_name_interned` | VtableEntry.fn_name Spur resolves correctly |
| 2 | `stage15_9_multiple_vtable_entries_interned` | Multiple entries all resolve |
| 3 | `stage15_9_trait_error_coherence_structured` | TraitError::Coherence carries CoherenceError |
| 4 | `stage15_9_trait_error_incomplete_structured` | TraitError::Incomplete carries IncompleteImpl |
| 5 | `stage15_9_trait_error_format_with_interner` | format_with_interner produces correct messages |
| 6 | `stage15_9_format_for_user_with_interner` | format_for_user displays trait errors |

## 3. Regression Test Strategy

### 3.1 Updated test files

27 test files were updated to use `interner.get_or_intern()` instead of
`.to_string()` when constructing VtableEntry. These tests verify the
VtableEntry construction still works correctly with the new Spur type.

### 3.2 Updated format_for_user callers

All `format_for_user` callers (in src/ and tests/) were updated to pass
`Some(&result.interner)` as the new interner parameter. These tests verify
the error display still works correctly.

### 3.3 Conformance tests

All 5216 conformance tests must continue to pass. The type changes are
transparent at the user-facing level — `compile()` and error display are
unchanged.

## 4. Coverage Matrix

| Module | Unit tests | Integration tests | Conformance |
|--------|-----------|-------------------|-------------|
| `VtableEntry.fn_name: Spur` | Existing vtable tests | 2 new + 27 updated | 5216 (all) |
| `TraitError` enum | N/A | 4 new | All pass |
| `format_with_interner` | N/A | 1 new | All pass |
| `format_for_user` with interner | N/A | 1 new + existing | All pass |
