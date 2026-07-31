# Stage 15.9 — VtableEntry.fn_name Interning + TraitError Typed Errors

> **Author**: redskaber
> **Date**: 2026-07-31
> **Version**: v0.134.0 → v0.135.0
> **Process**: stage-committee-process.md v3.23 §29 (stage-end deep review)
> **v0.2 Phase 1 Quick Wins**: HP-B16 + CoherenceError de-stringification

## 1. Executive Summary

Stage 15.9 completes the last two Phase 2 audit quick wins:

1. **Intern `VtableEntry.fn_name` to `Spur`** (HP-B16) — changed from
   `String` to `Spur`, eliminating per-entry heap allocation. For a crate
   with 50 trait methods, that's 50 fewer `String` allocations per
   compilation.

2. **Stop stringifying `CoherenceError`/`IncompleteImpl`** — changed
   `CompileErrors.trait_errors` from `Vec<String>` to `Vec<TraitError>`,
   preserving the structured `CoherenceError`/`IncompleteImpl` data. Added
   `TraitError::format_with_interner(interner)` for human-readable display.

Both changes are backward-compatible at the user-facing level (the
`compile()` API and error display are unchanged) but improve memory
efficiency and preserve structured error data for downstream consumers
(LSP, error reporters).

## 2. Changes Made

### 2.1 VtableEntry.fn_name: String → Spur

**Before** (`src/traits/vtable.rs`):
```rust
pub struct VtableEntry {
    pub method_name: Spur,
    pub fn_name: String,  // heap-allocated per entry
}
```

**After**:
```rust
pub struct VtableEntry {
    pub method_name: Spur,
    pub fn_name: Spur,  // interned — 8 bytes, no heap allocation
}
```

**Impact on consumers**:
- `TraitResolver::collect()` — now calls `interner.get_or_intern(...)` to
  intern the symbol name. Changed signature from `&Rodeo` to `&mut Rodeo`.
- `TraitResolver::resolve_vtable_method()` — added `interner: &Rodeo`
  parameter to resolve the Spur back to `&str`.
- `TraitResolver::vtable_method_names()` — same: added `interner` parameter.
- `TraitResolver::vtable_has_method()` — refactored to NOT need interner
  (uses new `find_vtable_method_entry` helper that returns `Option<&VtableEntry>`).
- `codegen::build_vtable_global_specs()` — resolves Spur via interner at
  the point of consumption.

**Why `find_vtable_method_entry` helper**: The existence check
(`vtable_has_method`) doesn't need to resolve the fn_name string — it
only needs to know if the entry exists. Extracting the entry lookup as a
separate helper avoids requiring the interner for boolean queries. Per
§1.0 原则 3 "显式 > 隐式": the entry lookup is explicit, separate from
the string resolution.

### 2.2 TraitError: Vec<String> → Vec<TraitError>

**Before** (`src/driver.rs`):
```rust
pub struct CompileErrors {
    pub trait_errors: Vec<String>,  // structured data lost
}
```

**After**:
```rust
pub struct CompileErrors {
    pub trait_errors: Vec<TraitError>,  // preserves CoherenceError/IncompleteImpl
}

pub enum TraitError {
    Coherence(CoherenceError),
    Incomplete(IncompleteImpl),
}

impl TraitError {
    pub fn format_with_interner(&self, interner: &Rodeo) -> String { ... }
}
```

**Impact on consumers**:
- `CompileErrors::format_for_user()` — added `interner: Option<&Rodeo>`
  parameter to resolve TraitError Spur symbols. Falls back to Debug
  formatting if interner is None (test contexts).
- `src/bin/main.rs` — passes `Some(&result.interner)`.
- `src/cargo.rs` — passes `Some(&result.interner)`.
- All test callers — updated to pass `Some(&result.interner)`.
- `TraitError` is re-exported from `lib.rs` for downstream consumers.

### 2.3 Driver simplification

The driver's trait error construction shrinks from ~20 lines of string
formatting to 4 lines of typed enum construction:

```rust
// Before: 20 lines of format!() calls
for ce in &validation_report.coherence_errors {
    let trait_str = interner.try_resolve(&ce.trait_name).unwrap_or("?");
    // ... 6 more lines ...
    errors.trait_errors.push(format!(...));
}

// After: 4 lines of typed construction
for ce in validation_report.coherence_errors {
    errors.trait_errors.push(TraitError::Coherence(ce));
}
for inc in validation_report.incomplete_impls {
    errors.trait_errors.push(TraitError::Incomplete(inc));
}
```

The formatting logic moves to `TraitError::format_with_interner`, which is
the single source of truth for human-readable trait error messages.

## 3. §29 Stage-End Deep Review

### 3.1 Data flow coverage (§29.1.1)

**VtableEntry.fn_name**: data flow is now `HIR → interner.get_or_intern() →
Spur → stored in VtableEntry → interner.try_resolve() → &str at consumption`.
The Spur is the canonical form; the &str is materialized only when needed
(for codegen symbol emission).

**TraitError**: data flow is now `validate_impls() → CoherenceError/IncompleteImpl
→ TraitError enum → stored in Vec<TraitError> → format_with_interner() →
String for display`. The structured data is preserved throughout; the
String is materialized only for human display.

No new catch-all branches. No silent error handling.

### 3.2 Architecture review (§29.1.2)

**Structural clarity** ✅ — `VtableEntry.fn_name` is now the same type as
`method_name` (both Spur), reflecting that both are interned symbols.
`TraitError` is a proper enum, not a string.

**Efficiency** ✅ — VtableEntry shrinks from ~40 bytes (String = 24 bytes
for ptr+len+cap) to ~16 bytes (Spur = 4 bytes × 2). For 50 vtable entries,
that's ~1.2KB saved per compilation.

**Extensibility** ✅ — adding a new TraitError variant (e.g., `ObjectSafety`)
means adding an enum variant + a format arm, not changing the type signature.

### 3.3 Design-impl-test coverage (§29.1.3)

| Design point | Implementation | Test |
|--------------|----------------|------|
| VtableEntry.fn_name is Spur | `vtable.rs` field type | `stage15_9_vtable_fn_name_interned` |
| Multiple entries resolve | `interner.try_resolve` loop | `stage15_9_multiple_vtable_entries_interned` |
| TraitError::Coherence structured | `TraitError::Coherence(CoherenceError)` | `stage15_9_trait_error_coherence_structured` |
| TraitError::Incomplete structured | `TraitError::Incomplete(IncompleteImpl)` | `stage15_9_trait_error_incomplete_structured` |
| format_with_interner correct | `TraitError::format_with_interner` | `stage15_9_trait_error_format_with_interner` |
| format_for_user with interner | `CompileErrors::format_for_user` | `stage15_9_format_for_user_with_interner` |

### 3.4 Hidden problems (§29.1.4)

| Hidden problem | Complexity growth | Stage 15.9 status |
|----------------|-------------------|-------------------|
| `TraitResolver::collect` now requires `&mut Rodeo` | 1× (API change) | Acceptable — driver already has `&mut` |
| `resolve_vtable_method` requires interner param | 1× (API change) | Acceptable — interner always available |
| `format_for_user` requires interner param | 1× (API change) | Acceptable — interner always available |

No new hidden problems. The API changes are surface-level (added params)
and don't introduce architectural debt.

### 3.5 Refactoring optimality (§29.2)

**Approach taken** ✅ — Both changes are pure type changes with no semantic
change. The VtableEntry interning uses the existing `Rodeo` interner (no
new interner). The TraitError enum reuses the existing `CoherenceError`/
`IncompleteImpl` structs (no new data structures).

**Alternative considered** ✅ — Could have kept `trait_errors: Vec<String>`
and added a separate `typed_trait_errors: Vec<TraitError>` field. Rejected
because it would duplicate the data and create confusion about which field
is authoritative.

**Skipped refactors** ✅ — Did not intern `Vtable.trait_name` and
`Vtable.self_ty_name` (they're already Spur). Did not change the
`ImplValidationReport` struct (it already uses typed errors). No
additional refactoring needed.

## 4. Test Results

| Test suite | Before (v0.134.0) | After (v0.135.0) | Delta |
|------------|-------------------|------------------|-------|
| Rust unit (lib) | 145 | 145 | 0 |
| Rust integration (all_tests) | 1970 | 1976 | +6 (Stage 15.9 tests) |
| Conformance (.lin) | 5216 | 5216 | 0 |
| **Total** | **7331** | **7337** | **+6** |

All tests pass. Zero regressions. Zero clippy warnings. fmt clean.

## 5. v0.2 Phase 1 Progress Update

| Task | Status | Notes |
|------|--------|-------|
| 1. Ty interning (`Ty<'tcx>` Copy) | Design done (Stage 15.1) | Implementation deferred to v0.3 |
| 2. SubstsRef → `&'tcx [GenericArg]` | Not started | Blocked on Task 1 |
| 3. TraitResolver key redesign | Not started | Blocked on Tasks 1+2 |
| 4. EmitValue → typed LLVM handle | Not started | Independent |
| 5. Consolidate 8 writeback passes → 2 | ✅ Done (Stage 15.7) | 650 LOC → 25 LOC |
| Quick win: AdtLayouts crate-level | ✅ Done (Stage 15.8) | ~500KB memory saved |
| **Quick win: VtableEntry.fn_name interning** | ✅ **Done (Stage 15.9)** | HP-B16 closed |
| **Quick win: Stop stringifying CoherenceError** | ✅ **Done (Stage 15.9)** | TraitError enum added |

Stage 15.9 closes the last two Phase 2 audit quick wins. All "4-hour"
quick wins are now complete. The next major milestone is v0.2 Phase 1
Task 1 (Ty interning via Rc stepping stone) — the biggest single
improvement for v0.2.

## 6. Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Version bump 0.134.0 → 0.135.0 |
| `src/traits/vtable.rs` | `VtableEntry.fn_name: String` → `Spur` |
| `src/traits/resolver.rs` | `collect()` takes `&mut Rodeo`; `resolve_vtable_method`/`vtable_method_names` take `interner` param; added `find_vtable_method_entry` helper; `vtable_has_method` no longer needs interner |
| `src/codegen/trait_dispatch/vtable.rs` | `build_vtable_global_specs` resolves Spur via interner |
| `src/driver.rs` | Added `TraitError` enum + `format_with_interner`; `trait_errors: Vec<String>` → `Vec<TraitError>`; `format_for_user` takes `interner` param; simplified trait error construction |
| `src/lib.rs` | Re-exported `TraitError` |
| `src/bin/main.rs` | Updated `format_for_user` call to pass interner |
| `src/cargo.rs` | Updated `format_for_user` call to pass interner |
| `tests/v0/stage5/plan/vtable_method_resolve_tests.rs` | Updated calls to pass interner |
| `tests/v0/stage5/plan/vtable_tests.rs` | Updated fn_name assertions to resolve via interner |
| `tests/v0/stage5/plan/trait_resolver_tests.rs` | Updated `collect` calls to take `&mut Rodeo` |
| `tests/v0/stage5/plan/driver_validation_tests.rs` | Updated to use `format_with_interner` |
| 22 other test files | Updated VtableEntry construction to use `interner.get_or_intern()` |
| `tests/v0/stage3/plan/codegen_tests.rs` | Updated `format_for_user` calls |
| `tests/v0/stage2/plan/integration_tests.rs` | Updated `format_for_user` calls |
| `examples/usage/trait_dispatch_emission.rs` | Updated `format_for_user` call |
| `tests/v0/stage15/plan/vtable_interning_and_trait_error_tests.rs` | **NEW** — 6 integration tests |
| `tests/all_tests.rs` | Registered `stage15_vtable_interning_and_trait_error_tests` |
| `docs/develop/v0/stage-15/stage-15.9-vtable-interning-trait-error.md` | This document |
| `docs/tests/v0/stage15/stage-15.9-test-plan.md` | **NEW** — test plan |
| `docs/worklog.md` | Stage 15.9 entry appended |
| `RELEASE_NOTES.md` | v0.135.0 entry appended |
| `README.md` | Updated with Stage 15.9 progress |
