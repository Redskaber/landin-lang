# Stage 15.8 — Test Plan

> **Date**: 2026-07-31
> **Version**: v0.133.0 → v0.134.0
> **Process**: stage-committee-process.md v3.23 §17 + §29.1.3

## 1. Test Scope

Stage 15.8 changes the `AdtLayouts` storage from per-body (owned HashMap)
to crate-level (shared `Arc<AdtLayouts>`). The test plan covers:

| Area | Test type | Count |
|------|-----------|-------|
| `build_crate_adt_layouts` | Integration (real HIR) | 6 new |
| `Arc<AdtLayouts>` sharing | Integration (Arc::ptr_eq) | 1 (in above) |
| Regression (existing features) | Conformance + Rust integration | 5216 + 1964 (unchanged) |

## 2. Integration Test Module

**Path**: `tests/v0/stage15/plan/crate_adt_layouts_tests.rs`
**Registered as**: `stage15_crate_adt_layouts_tests` in `tests/all_tests.rs`

### 2.1 Test cases

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_8_struct_layout_crate_level` | Struct layout registered, shared across bodies |
| 2 | `stage15_8_enum_layout_crate_level` | Enum layout registered crate-level |
| 3 | `stage15_8_nested_struct_layouts` | Nested ADT layouts registered recursively |
| 4 | `stage15_8_layouts_shared_across_bodies` | All bodies share the SAME Arc (ptr equality) |
| 5 | `stage15_8_struct_return_method_call_regression` | Struct-returning method calls work |
| 6 | `stage15_8_array_of_structs_regression` | Array of structs works |

### 2.2 Test design rationale

All tests use `compile()` to run the full pipeline with real HIR. This
verifies the crate-level AdtLayouts works correctly end-to-end.

Test 4 (`stage15_8_layouts_shared_across_bodies`) is the key test for
the Arc sharing: it uses `Arc::ptr_eq` to verify that all MirBodies
share the same `Arc<AdtLayouts>` (not just equal-by-value, but
pointer-identical).

Tests 5 and 6 are regression tests for the patterns that originally
required the "re-populate after writeback" hack (Stages 14.41, 14.84).
They verify the crate-level map correctly supports these patterns
without the per-body re-runs.

## 3. Regression Test Strategy

### 3.1 Conformance tests

All 5216 conformance tests must continue to pass. The Arc sharing is
transparent — codegen reads `&*mir.adt_layouts` which derefs to
`&AdtLayouts`, the same type as before.

### 3.2 Rust integration tests

All 1964 existing integration tests must continue to pass. Run with:

```bash
cargo test --features llvm-backend
```

Expected: 1970 passed (1964 + 6 new), 0 failed, 2 ignored.

## 4. Coverage Matrix

| Module | Unit tests | Integration tests | Conformance |
|--------|-----------|-------------------|-------------|
| `build_crate_adt_layouts` | 0 (covered by integration) | 6 (real HIR) | 5216 (all) |
| `MirBody.adt_layouts: Arc<AdtLayouts>` | Existing body.rs tests | 6 (Arc::ptr_eq) | All pass |
| `driver.rs` (crate-level sharing) | N/A | All pass | All pass |
| `codegen` (Arc deref) | N/A | All pass | All pass |

## 5. Test File Location

```
tests/
└── v0/
    └── stage15/
        └── plan/
            ├── method_return_type_cache_tests.rs   (Stage 15.6)
            ├── writeback_consolidation_tests.rs     (Stage 15.7)
            └── crate_adt_layouts_tests.rs           # NEW (Stage 15.8)
```
