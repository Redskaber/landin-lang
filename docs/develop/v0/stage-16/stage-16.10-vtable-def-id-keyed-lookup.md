# Stage 16.10 — Task 3 Step 3 Continuation: Vtable DefId-Keyed Lookup + Stage Directory Restructure

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.227.3 → v0.227.4
> **Process**: stage-committee-process.md v3.24 §1.0 原則 6 "通用 > 特例" + §23 API 命名标准化 + §16 接口隔离

## 1. Executive Summary

Stage 16.10 continues Task 3 Step 3 by migrating vtable lookup from
Spur-keyed to DefId-keyed. It also reorganizes Stage 16 documentation
and tests into proper `stage-16` directories per the user's directive.

**Key changes**:
1. **Directory restructure**: Created `docs/develop/v0/stage-16/` and
   `tests/v0/stage16/plan/` directories. Moved all Stage 16 docs and
   tests from `stage-15/` to `stage-16/`.
2. Added `vtables_by_def_ids: HashMap<(DefId, DefId), Vtable>` to `TraitResolver`.
3. Added `find_vtable_by_def_ids(trait_def_id, self_type_def_id)` method.
4. Added `populate_def_id_keyed_maps()` post-pass in `collect()` to handle
   HIR iteration ordering (user-defined traits may appear after impls).
5. Migrated `dyn_trait.rs` vtable lookup to DefId-keyed (with Spur fallback
   for test contexts that construct TraitResolver manually).
6. +7 integration tests.

**Result**: Vtable lookup is now DefId-keyed in the production path.
The Spur-keyed `vtables` map remains for backward compatibility and for
callers that need string names (e.g., `build_dyn_trait_fat_ptrs_from_resolver`).

## 2. Directory Restructure

Per user directive: "当进入新 stage 时需要（develop/ 、tests/）同步创建 stage 目录管理"

### Before
```
docs/develop/v0/stage-15/stage-16.00-v0.3-kickoff.md
docs/develop/v0/stage-15/stage-16.01-...
...
tests/v0/stage15/plan/stage16_05_*.rs
tests/v0/stage15/plan/stage16_06_*.rs
...
```

### After
```
docs/develop/v0/stage-16/stage-16.00-v0.3-kickoff.md
docs/develop/v0/stage-16/stage-16.01-...
...
tests/v0/stage16/plan/stage16_05_*.rs
tests/v0/stage16/plan/stage16_06_*.rs
...
```

Updated `tests/all_tests.rs` path references from `v0/stage15/plan/stage16_*`
to `v0/stage16/plan/stage16_*`.

## 3. Implementation

### 3.1 New Field: `vtables_by_def_ids`

```rust
pub struct TraitResolver {
    // ... existing fields ...
    /// Stage 16.10: Vtables keyed by (trait_def_id, self_type_def_id).
    pub vtables_by_def_ids: HashMap<(DefId, DefId), Vtable>,
}
```

### 3.2 `populate_def_id_keyed_maps()` Post-Pass

The inline population during `collect()` failed for user-defined traits
when the impl block was processed BEFORE the trait definition (due to
HashMap iteration order). The post-pass runs after ALL traits, types,
and impls have been collected, ensuring all lookups succeed:

```rust
fn populate_def_id_keyed_maps(&mut self) {
    self.impls_by_def_ids.clear();
    self.vtables_by_def_ids.clear();
    for (impl_def_id, info) in &self.impls {
        // Resolve trait_name Spur → trait DefId
        // Resolve self_ty_name Spur → self type DefId
        // Populate impls_by_def_ids and vtables_by_def_ids
    }
}
```

This also fixes a latent bug in Stage 16.07's `impls_by_def_ids` — the
inline population had the same ordering issue for user-defined traits,
but it wasn't caught because Stage 16.07's tests only used builtin traits
(Copy, Drop) which are pre-registered.

### 3.3 `find_vtable_by_def_ids()` Method

```rust
pub fn find_vtable_by_def_ids(
    &self,
    trait_def_id: DefId,
    self_type_def_id: DefId,
) -> Option<&Vtable> {
    self.vtables_by_def_ids.get(&(trait_def_id, self_type_def_id))
}
```

### 3.4 `dyn_trait.rs` Migration

The `build_dyn_trait_method_calls_from_resolver` function was migrated
to use DefId-keyed vtable lookup, with a Spur-based fallback for test
contexts that construct TraitResolver manually without calling `collect()`:

```rust
let vtable_opt = {
    // Try DefId-keyed lookup first (preferred path).
    let def_id_vtable = trait_resolver
        .find_trait_def_id(tn)
        .and_then(|trait_def_id| { ... find_vtable_by_def_ids ... });
    // Fall back to Spur-based lookup if DefId-keyed fails.
    def_id_vtable.or_else(|| trait_resolver.vtables.get(&(tn, ty)))
};
```

## 4. API Naming Standard Compliance (§23)

| Method/Field | Pattern | Status |
|--------------|---------|--------|
| `vtables_by_def_ids` | `<noun>_<prep>_<noun>` | ✅ |
| `find_vtable_by_def_ids` | `<verb>_<noun>_<prep>_<noun>` | ✅ |
| `populate_def_id_keyed_maps` | `<verb>_<noun>_<noun>` | ✅ |
| `_by_def_ids` suffix | Consistent with `impls_by_def_ids` (Stage 16.07) | ✅ |

## 5. §16 Interface Isolation Compliance

- `TraitResolver::collect()` + `populate_def_id_keyed_maps()` read internal maps (no HIR access in post-pass).
- `find_vtable_by_def_ids` is a pure query method (no side effects).
- `dyn_trait.rs` queries via `find_vtable_by_def_ids` (no HIR access).
- Spur-based fallback is transparent — callers don't need to know which path was used.

## 6. Tests

Added `tests/v0/stage16/plan/stage16_10_vtable_def_id_lookup_tests.rs`
with 7 tests:
1. `find_vtable_by_def_ids` returns vtable for user-defined trait
2. `find_vtable_by_def_ids` returns None for no impl
3. `vtables_by_def_ids` map is populated during collect
4. DefId-keyed and Spur-based vtable lookups agree (consistency)
5. `find_vtable_by_def_ids` works with multiple methods (ordering)
6. Post-pass handles user-defined trait HIR ordering
7. dyn Trait method calls work with DefId-keyed vtable

## 7. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2191/2191 PASS (+7 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7659 tests passing, 0 failures, 0 warnings.**

## 8. Version Policy

v0.227.3 → v0.227.4 (patch bump — new method + directory restructure,
no behavior change for valid programs. The dyn_trait.rs migration is
backward compatible due to the Spur fallback.)

## 9. Task 3 Roadmap Update

| Step | Status | Description |
|------|--------|-------------|
| Step 1 | ✅ COMPLETE (Stage 16.07) | DefId-keyed impl lookup |
| Step 2 | 🔧 Pending | Add SubstsRef to keys for generics |
| Step 3 | ✅ **COMPLETE** (Stages 16.08 + 16.10) | Builtin trait checks + vtable migration |
| Step 4 | 🔧 Pending | Deprecate Spur-based methods |

**Next**: Task 3 Step 4 (deprecate Spur-based `find_impl`/`implements`/
`find_vtable` methods) can now proceed — all production callers use
DefId-keyed lookups. The Spur-based methods remain only for:
- `build_dyn_trait_fat_ptrs_from_resolver` (needs string names)
- Test contexts (manual TraitResolver construction)
- Defensive fallbacks
