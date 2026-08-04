# Stage 16.07 — Task 3 Step 1: DefId-Keyed Trait Impl Lookup

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.227.0 → v0.227.1
> **Process**: stage-committee-process.md v3.24 §1.0 原則 6 "通用 > 特例" + §23 API 命名标准化 + §16 接口隔离

## 1. Executive Summary

Stage 16.07 is **Step 1 of Task 3** (TraitResolver Keys redesign). It
introduces DefId-keyed trait impl lookup alongside the existing
Spur-based lookup, preparing for generic SubstsRef support (Task 3
Step 2).

**Key changes**:
1. Added `impls_by_def_ids: HashMap<(DefId, DefId), DefId>` to `TraitResolver`.
2. Added `find_impl_by_def_ids(trait_def_id, self_type_def_id)` method.
3. Added `implements_by_def_ids(trait_def_id, self_type_def_id)` method.
4. Added `find_trait_def_id(trait_name_spur)` helper.
5. Migrated codegen's Drop impl lookup to use the new DefId-keyed method.
6. +9 integration tests.

**Result**: Type-safe, interner-free trait impl lookup. Backward
compatible — old Spur-based methods retained. Prepares for Task 3
Step 2 (SubstsRef keys for generics).

## 2. Background

### 2.1 The Problem

The current `TraitResolver` keys impls by `(Spur, Spur)` —
`(trait_name_spur, type_name_spur)`:

```rust
pub impl_by_trait_and_type: HashMap<(Spur, Spur), DefId>,
```

This has three problems:
1. **Not type-safe**: Spur is a string hash, not a unique identifier.
   Two different traits with the same name (in different modules)
   would collide.
2. **Requires interner**: callers need `&Rodeo` to resolve names to
   Spurs, adding a dependency.
3. **No generics support**: `Vec<i32>` and `Vec<bool>` have the same
   Spur "Vec", so they can't have different impls.

### 2.2 Task 3 Goal

Task 3 redesigns the key from `(Spur, Spur)` to `(DefId, SubstsRef)`:
- `DefId` is a unique identifier (type-safe).
- `SubstsRef` (Rc<[Ty]>) carries generic type arguments.
- This unblocks Tasks 11 (Monomorphization), 14 (Object safety), 17
  (Associated types).

### 2.3 Step 1 Scope

Step 1 (this stage) introduces the DefId-keyed lookup **without**
SubstsRef. For v0.1 (no generics), `SubstsRef` is always empty, so
DefId-only keys are sufficient. Step 2 (future) will extend the key
to `(DefId, SubstsRef)` when generic support is added.

## 3. Implementation

### 3.1 New Field: `impls_by_def_ids`

```rust
pub struct TraitResolver {
    // ... existing fields ...
    /// Stage 16.07 (Task 3 step 1): (trait_def_id, self_type_def_id) →
    /// impl DefId. DefId-keyed lookup — type-safe, no interner needed.
    pub impls_by_def_ids: HashMap<(DefId, DefId), DefId>,
}
```

### 3.2 Population in `collect()`

During `collect()`, when an impl block is processed, the new map is
populated alongside the existing Spur-based map:

```rust
if let Some(trait_def_id) = self.trait_by_name.get(&tn).copied() {
    let self_def_id = self.type_by_def_id
        .iter()
        .find(|(_, &name)| name == stn)
        .map(|(&d, _)| d);
    if let Some(self_def_id) = self_def_id {
        self.impls_by_def_ids.insert((trait_def_id, self_def_id), *def_id);
    }
}
```

### 3.3 New Query Methods

```rust
/// Look up an impl block by DefIds.
pub fn find_impl_by_def_ids(
    &self,
    trait_def_id: DefId,
    self_type_def_id: DefId,
) -> Option<&ImplInfo> {
    self.impls_by_def_ids
        .get(&(trait_def_id, self_type_def_id))
        .and_then(|id| self.impls.get(id))
}

/// Check if a type implements a trait, keyed by DefIds.
pub fn implements_by_def_ids(
    &self,
    trait_def_id: DefId,
    self_type_def_id: DefId,
) -> bool {
    self.find_impl_by_def_ids(trait_def_id, self_type_def_id).is_some()
}

/// Look up a trait DefId by name.
pub fn find_trait_def_id(&self, trait_name: Spur) -> Option<DefId> {
    self.trait_by_name.get(&trait_name).copied()
}
```

### 3.4 Codegen Migration

The codegen Drop glue emission (`emit_drop_glue_functions`) was
migrated to use the new DefId-keyed lookup:

```rust
// Before:
let has_drop_impl = drop_name
    .map(|dn| resolver.implements(dn, type_spur))
    .unwrap_or(false);

// After:
let has_drop_impl = if let Some(drop_did) = drop_def_id {
    resolver.implements_by_def_ids(drop_did, def_id)
} else {
    // Defensive fallback: Spur-based lookup.
    interner.get("Drop")
        .map(|dn| resolver.implements(dn, type_spur))
        .unwrap_or(false)
};
```

The fallback ensures backward compatibility if the Drop trait isn't
registered (shouldn't happen after `register_builtin_traits`).

## 4. API Naming Standard Compliance (§23)

| Rule | Compliance |
|------|------------|
| §23.1.1 Free function entry | ✅ Methods on TraitResolver (stateful) |
| §23.1.2 Context type naming | ✅ `TraitResolver` follows `-er` suffix |
| §23.1.3 Type prefix | ✅ No new types needed |
| §23.1.4 Re-export style | N/A (no re-export changes) |
| §23.1.5 DRY | ✅ Single source of truth for DefId-keyed lookup |
| §23.1.6 Deprecation | N/A (old methods retained, not deprecated yet) |
| §23.1.7 Function naming prefix | ✅ `find_impl_by_def_ids`, `implements_by_def_ids`, `find_trait_def_id` |
| §23.1.8 Error type suffix | N/A (no new error types) |

Method naming:
- `find_impl_by_def_ids` — `<verb>_<noun>_<prep>_<noun>` pattern
- `implements_by_def_ids` — `<verb>_<prep>_<noun>` pattern
- `find_trait_def_id` — `<verb>_<noun>_<noun>` pattern
- `_by_def_ids` suffix distinguishes from Spur-based `find_impl`/`implements`

## 5. §16 Interface Isolation Compliance

- `TraitResolver::collect()` reads HIR (allowed — data flows downstream).
- New methods are on `TraitResolver` (same as existing methods).
- `impls_by_def_ids` is `pub` for testing and potential future use.
- No new HIR access from borrowck/codegen.

## 6. Tests

Added `tests/v0/stage15/plan/stage16_07_def_id_keyed_lookup_tests.rs`
with 9 tests:
1. `find_trait_def_id` returns the trait's DefId
2. `find_trait_def_id` returns None for unknown trait
3. `implements_by_def_ids` finds existing impl
4. `implements_by_def_ids` returns false for no impl
5. DefId-keyed and Spur-based lookups agree (consistency)
6. `find_impl_by_def_ids` returns the impl info
7. `impls_by_def_ids` map is populated during collect
8. Copy trait works with DefId-keyed lookup
9. User-defined trait works with DefId-keyed lookup

## 7. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2169/2169 PASS (+9 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7637 tests passing, 0 failures, 0 warnings.**

## 8. Version Policy

v0.227.0 → v0.227.1 (patch bump — new methods added, no behavior change.
The codegen migration is a refactor; results are identical.)

## 9. Task 3 Roadmap

| Step | Status | Description |
|------|--------|-------------|
| Step 1 | ✅ COMPLETE (Stage 16.07) | DefId-keyed lookup, no SubstsRef |
| Step 2 | 🔧 Pending | Add SubstsRef to keys for generic support |
| Step 3 | 🔧 Pending | Migrate all Spur-based callers to DefId-keyed |
| Step 4 | 🔧 Pending | Deprecate Spur-based methods |

**Next**: Task 3 Step 2 (SubstsRef keys) requires generic type support
in the parser/HIR, which is a larger effort. Alternatively, Task 11
(Monomorphization) can start once Step 1 is done, using the DefId-keyed
lookup as the foundation.
