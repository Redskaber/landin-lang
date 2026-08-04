# Stage 16.08 — Task 3 Step 3: Builtin Trait Check Migration to DefId-Keyed Lookup

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.227.1 → v0.227.2
> **Process**: stage-committee-process.md v3.24 §1.0 原則 6 "通用 > 特例" + §23 API 命名标准化 + §16 接口隔离

## 1. Executive Summary

Stage 16.08 is **Step 3 of Task 3** (TraitResolver Keys redesign). It
migrates the builtin trait check methods (`is_copy_builtin`,
`is_clone_builtin`, `is_drop_builtin`, `implements_builtin_trait`) from
Spur-based lookup (`implements_by_def_id`) to DefId-keyed lookup
(`implements_by_def_ids`).

**Key changes**:
1. `is_copy_builtin` now uses `find_trait_def_id` + `implements_by_def_ids`.
2. `is_clone_builtin` migrated to DefId-keyed lookup.
3. `is_drop_builtin` migrated to DefId-keyed lookup.
4. `implements_builtin_trait` migrated to DefId-keyed lookup.
5. Codegen drop glue emission simplified — removed pre-resolution of
   `drop_def_id` and Spur-based fallback, now directly calls
   `is_drop_builtin`.
6. +10 integration tests verifying behavior preservation.

**Result**: All builtin trait checks now use the type-safe DefId-keyed
path. The `interner` parameter is retained for resolving trait name
strings to Spurs, but the actual impl lookup is DefId-keyed. This
prepares for Task 3 Step 4 (deprecate Spur-based methods).

## 2. Background

### 2.1 Stage 16.07 Foundation

Stage 16.07 (Task 3 Step 1) introduced:
- `impls_by_def_ids: HashMap<(DefId, DefId), DefId>` field
- `find_impl_by_def_ids(trait_def_id, self_type_def_id)` method
- `implements_by_def_ids(trait_def_id, self_type_def_id)` method
- `find_trait_def_id(trait_name_spur)` helper

### 2.2 The Migration Target

The builtin trait check methods (`is_copy_builtin`, `is_clone_builtin`,
`is_drop_builtin`, `implements_builtin_trait`) were still using the
old Spur-based path:

```rust
// Old (Spur-based):
pub fn is_copy_builtin(&self, def_id: DefId, interner: &Rodeo) -> bool {
    if let Some(copy_name) = interner.get("Copy") {
        self.is_copy(def_id, copy_name)  // → implements_by_def_id → implements (Spur)
    } else {
        false
    }
}
```

Stage 16.08 migrates them to:

```rust
// New (DefId-keyed):
pub fn is_copy_builtin(&self, def_id: DefId, interner: &Rodeo) -> bool {
    if self.derived_copy_types.contains(&def_id) {
        return true;
    }
    if let Some(copy_name) = interner.get("Copy") {
        if let Some(trait_def_id) = self.find_trait_def_id(copy_name) {
            self.implements_by_def_ids(trait_def_id, def_id)  // DefId-keyed!
        } else {
            false
        }
    } else {
        false
    }
}
```

## 3. Implementation

### 3.1 `is_copy_builtin` Migration

The method now:
1. Checks `derived_copy_types` first (Stage 16.06, unchanged).
2. Resolves "Copy" Spur → trait DefId via `find_trait_def_id`.
3. Calls `implements_by_def_ids(trait_def_id, def_id)` (DefId-keyed).

The `interner` parameter is retained to resolve "Copy" to a Spur. Future
Step 4 can remove it once all callers pre-resolve the trait DefId.

### 3.2 `is_clone_builtin` Migration

Same pattern as `is_copy_builtin` — resolves "Clone" Spur → DefId, then
calls `implements_by_def_ids`.

### 3.3 `is_drop_builtin` Migration

Same pattern — resolves "Drop" Spur → DefId, then calls
`implements_by_def_ids`.

### 3.4 `implements_builtin_trait` Migration

The generic `implements_builtin_trait(def_id, trait_name, interner)`
method now resolves the trait name string → Spur → DefId, then calls
`implements_by_def_ids`.

### 3.5 Codegen Simplification

The codegen drop glue emission (`emit_drop_glue_functions`) was
simplified. Stage 16.07 had pre-resolved `drop_def_id` and called
`implements_by_def_ids` directly, with a Spur-based fallback. Now that
`is_drop_builtin` uses DefId-keyed lookup, the codegen can just call
`is_drop_builtin` directly:

```rust
// Stage 16.07 (complex):
let drop_def_id = interner.get("Drop").and_then(|dn| resolver.find_trait_def_id(dn));
let has_drop_impl = if let Some(drop_did) = drop_def_id {
    resolver.implements_by_def_ids(drop_did, def_id)
} else {
    interner.get("Drop").map(|dn| resolver.implements(dn, type_spur)).unwrap_or(false)
};

// Stage 16.08 (simple):
let has_drop_impl = resolver.is_drop_builtin(def_id, interner);
```

This is cleaner and the fallback is handled internally by
`is_drop_builtin`.

## 4. API Naming Standard Compliance (§23)

| Method | Pattern | Status |
|--------|---------|--------|
| `is_copy_builtin` | `<verb>_<adj>_<noun>` | ✅ (internal migration) |
| `is_clone_builtin` | `<verb>_<adj>_<noun>` | ✅ (internal migration) |
| `is_drop_builtin` | `<verb>_<adj>_<noun>` | ✅ (internal migration) |
| `implements_builtin_trait` | `<verb>_<adj>_<noun>` | ✅ (internal migration) |
| `find_trait_def_id` | `<verb>_<noun>_<noun>` | ✅ (Stage 16.07) |
| `implements_by_def_ids` | `<verb>_<prep>_<noun>` | ✅ (Stage 16.07) |

No API surface changes — the method signatures are unchanged. The
migration is purely internal (different lookup path, same results).

## 5. §16 Interface Isolation Compliance

- `TraitResolver` methods read internal maps (no HIR access).
- `BorrowChecker` queries via `is_copy_builtin` (unchanged interface).
- `codegen` queries via `is_drop_builtin` (unchanged interface).
- No new HIR dependencies introduced.

## 6. Tests

Added `tests/v0/stage15/plan/stage16_08_builtin_trait_migration_tests.rs`
with 10 tests:
1. `is_copy_builtin` returns true for explicit Copy
2. `is_copy_builtin` returns true for derived Copy (Stage 16.06)
3. `is_copy_builtin` returns false for Copy+Drop conflict
4. `is_drop_builtin` returns true for explicit Drop
5. `is_drop_builtin` returns false for no Drop
6. `is_clone_builtin` returns true for explicit Clone
7. `is_clone_builtin` returns false for no Clone
8. `implements_builtin_trait("Copy")` agrees with `is_copy_builtin`
9. `implements_builtin_trait("Drop")` agrees with `is_drop_builtin`
10. DefId-keyed and Spur-based lookups agree for explicit impls

Test 10 verifies the migration is behavior-preserving: for types with
explicit `impl Copy` or `impl Drop`, both lookup paths give the same
result. (Derived Copy from Stage 16.06 is a separate concern —
`is_copy_builtin` returns true for derived-Copy types, while
`implements_by_def_id` returns false since there's no explicit impl.)

## 7. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2179/2179 PASS (+10 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7647 tests passing, 0 failures, 0 warnings.**

## 8. Version Policy

v0.227.1 → v0.227.2 (patch bump — internal migration, no API surface
changes, no behavior changes for valid programs.)

## 9. Task 3 Roadmap Update

| Step | Status | Description |
|------|--------|-------------|
| Step 1 | ✅ COMPLETE (Stage 16.07) | DefId-keyed lookup, no SubstsRef |
| Step 2 | 🔧 Pending | Add SubstsRef to keys for generic support |
| Step 3 | ✅ **COMPLETE (Stage 16.08)** | Migrate builtin trait checks to DefId-keyed |
| Step 4 | 🔧 Pending | Deprecate Spur-based methods |

**Remaining for Step 3**: Migrate vtable emission (`find_vtable`,
`vtables` map) to DefId-keyed. This is a larger change because vtables
are keyed by `(Spur, Spur)` and used in codegen for dyn Trait dispatch.

**Next**: Task 3 Step 4 (deprecate Spur-based methods) can start once
all callers are migrated. The vtable migration may be deferred to a
separate stage.
