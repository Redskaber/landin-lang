# Task 3: TraitResolver Keys Redesign — Design Document

> **Author**: redskaber
> **Date**: 2026-08-03
> **Status**: Step 1 COMPLETE (Stage 16.07), Steps 2-4 pending
> **Process**: stage-committee-process.md v3.24 §1.0 原則 6 "通用 > 特例" + §13.4 (数据结构选型) + §16 (接口隔离)

## 1. Executive Summary

Task 3 redesigns `TraitResolver`'s impl lookup keys from `(Spur, Spur)`
to `(DefId, SubstsRef)`. This is a prerequisite for:
- Task 11 (Monomorphization) — generic type instantiation
- Task 14 (Object safety) — dyn Trait with generics
- Task 17 (Associated types) — type-level computation

**Current state**: Step 1 complete (Stage 16.07). DefId-keyed lookup
introduced alongside Spur-based lookup. Backward compatible.

## 2. Problem Statement

### 2.1 Current Architecture (v0.227.0)

```rust
pub struct TraitResolver {
    /// (trait_name_spur, self_ty_name_spur) → impl DefId
    pub impl_by_trait_and_type: HashMap<(Spur, Spur), DefId>,
    // ...
}
```

Lookup methods:
- `find_impl(trait_name: Spur, self_ty_name: Spur) -> Option<&ImplInfo>`
- `implements(trait_name: Spur, self_ty_name: Spur) -> bool`
- `implements_by_def_id(trait_name: Spur, def_id: DefId) -> bool`
  (converts DefId → Spur via `type_by_def_id`, then calls `implements`)

### 2.2 Problems

1. **Not type-safe**: `Spur` is a string hash. Two traits with the same
   name in different modules would collide. (Not an issue in v0.1 which
   has no modules, but will be in v0.2+.)

2. **Requires interner**: Callers need `&Rodeo` to resolve names to
   Spurs. This adds a dependency and makes the API less ergonomic.

3. **No generics support**: `Vec<i32>` and `Vec<bool>` both have Spur
   "Vec", so they can't have different impls. This blocks
   monomorphization.

4. **Indirect for DefId callers**: `implements_by_def_id` does a
   reverse lookup (DefId → Spur → impl), which is O(n) in the number
   of types.

### 2.3 Goals

- **Type safety**: Use `DefId` (unique identifier) instead of `Spur`.
- **No interner dependency**: Callers don't need `&Rodeo`.
- **Generic support**: Key includes `SubstsRef` for type arguments.
- **Performance**: Direct lookup, no reverse mapping.
- **Backward compatible**: Old methods retained during migration.

## 3. Design

### 3.1 New Key Structure

```rust
/// Stage 16.07+: DefId-keyed impl lookup.
/// (trait_def_id, self_type_def_id) → impl DefId.
/// Step 2 will extend to (DefId, SubstsRef) for generics.
pub impls_by_def_ids: HashMap<(DefId, DefId), DefId>,
```

**Step 2** (future) will change the key to:
```rust
pub impls_by_def_id_and_substs: HashMap<(DefId, DefId, SubstsRef), DefId>,
```

Or more idiomatically:
```rust
pub impls_by_def_id_and_substs: HashMap<TraitImplKey, DefId>,

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraitImplKey {
    pub trait_def_id: DefId,
    pub self_type_def_id: DefId,
    pub substs: SubstsRef,  // Rc<[Ty]>
}
```

### 3.2 New Methods (Step 1 — implemented)

```rust
/// DefId-keyed impl lookup.
pub fn find_impl_by_def_ids(
    &self,
    trait_def_id: DefId,
    self_type_def_id: DefId,
) -> Option<&ImplInfo>

/// DefId-keyed implements check.
pub fn implements_by_def_ids(
    &self,
    trait_def_id: DefId,
    self_type_def_id: DefId,
) -> bool

/// Convert trait name Spur to DefId.
pub fn find_trait_def_id(&self, trait_name: Spur) -> Option<DefId>
```

### 3.3 Step 2 Methods (planned)

```rust
/// DefId + SubstsRef-keyed impl lookup (for generics).
pub fn find_impl_with_substs(
    &self,
    trait_def_id: DefId,
    self_type_def_id: DefId,
    substs: SubstsRef,
) -> Option<&ImplInfo>

/// DefId + SubstsRef-keyed implements check.
pub fn implements_with_substs(
    &self,
    trait_def_id: DefId,
    self_type_def_id: DefId,
    substs: SubstsRef,
) -> bool
```

For non-generic types, `substs` is empty (`Rc::new([])`), and the
result is identical to `find_impl_by_def_ids`.

## 4. Migration Plan

### 4.1 Step 1 (COMPLETE — Stage 16.07)

- Add `impls_by_def_ids` field.
- Add `find_impl_by_def_ids`, `implements_by_def_ids`, `find_trait_def_id`.
- Populate during `collect()`.
- Migrate codegen's Drop lookup.
- +9 integration tests.

### 4.2 Step 2 (Pending — requires generic parser support)

- Add `SubstsRef` to the key.
- Add `find_impl_with_substs`, `implements_with_substs`.
- Requires: parser support for generic types (`Vec<T>`).
- Requires: HIR `HirTyKind::Path` carrying generic args.
- Requires: MIR `TyKind::Adt(DefId, SubstsRef)` populated with real substs.

**Effort**: 1-2 weeks (depends on generic parser work).

### 4.3 Step 3 (Pending — after Step 2)

- Migrate all Spur-based callers to DefId-keyed:
  - `borrowck/copy_semantics.rs` — `is_copy_builtin` → use `implements_by_def_ids`
  - `borrowck/copy_semantics.rs` — `ty_is_copy_with_resolver` → use DefId
  - `mir/drop_elaboration.rs` — `is_drop_builtin` → use `implements_by_def_ids`
  - `codegen/mod.rs` — vtable emission → use DefId-keyed
  - `driver.rs` — trait validation → use DefId-keyed

**Effort**: 3-5 days.

### 4.4 Step 4 (Pending — after Step 3)

- Deprecate Spur-based methods:
  - `find_impl(trait_name: Spur, self_ty_name: Spur)` → `#[deprecated]`
  - `implements(trait_name: Spur, self_ty_name: Spur)` → `#[deprecated]`
  - `implements_by_def_id(trait_name: Spur, def_id: DefId)` → `#[deprecated]`
- Keep `impl_by_trait_and_type` map for backward compat (or remove if
  all callers migrated).

**Effort**: 1-2 days.

## 5. Data Structure Selection (§13.4)

### 5.1 Key Type

**Step 1**: `(DefId, DefId)` — tuple, no allocation.
**Step 2**: `(DefId, DefId, SubstsRef)` or `TraitImplKey` struct.

`TraitImplKey` struct is preferred because:
- Named fields are clearer than tuple positions.
- Can derive `Eq, Hash` for HashMap key.
- Can add methods (e.g., `is_generic()`, `without_substs()`).

### 5.2 Map Type

`HashMap<Key, DefId>` — O(1) average lookup.

Alternative: `BTreeMap<Key, DefId>` — O(log n) but ordered iteration.
Not needed unless we want ordered iteration (we don't).

### 5.3 SubstsRef

Already exists as `Rc<[Ty]>` (Stage 15.10). Using `Rc` makes cloning
cheap (refcount bump), and allows sharing the same substs slice across
multiple lookups.

## 6. §16 Interface Isolation Compliance

- `TraitResolver::collect()` reads HIR (allowed — downstream data flow).
- Query methods are on `TraitResolver` (no HIR access needed).
- `BorrowChecker` queries via `is_copy_builtin` → `implements_by_def_ids`.
- `codegen` queries via `implements_by_def_ids` directly.

## 7. §23 API Naming Compliance

| Method | Pattern | Status |
|--------|---------|--------|
| `find_impl_by_def_ids` | `<verb>_<noun>_<prep>_<noun>` | ✅ Step 1 |
| `implements_by_def_ids` | `<verb>_<prep>_<noun>` | ✅ Step 1 |
| `find_trait_def_id` | `<verb>_<noun>_<noun>` | ✅ Step 1 |
| `find_impl_with_substs` | `<verb>_<noun>_<prep>_<noun>` | 🔧 Step 2 |
| `implements_with_substs` | `<verb>_<prep>_<noun>` | 🔧 Step 2 |

## 8. Testing Strategy

### 8.1 Step 1 Tests (COMPLETE — 9 tests)

- `find_trait_def_id` returns correct DefId
- `implements_by_def_ids` finds existing impl
- `implements_by_def_ids` returns false for no impl
- DefId-keyed and Spur-based lookups agree (consistency)
- `find_impl_by_def_ids` returns impl info
- Map is populated during collect
- Copy/Drop/user-defined traits work

### 8.2 Step 2 Tests (planned)

- `find_impl_with_substs` with empty substs = `find_impl_by_def_ids`
- `find_impl_with_substs` with non-empty substs (generic types)
- `Vec<i32>` and `Vec<bool>` have different impls (if user defines them)

### 8.3 Step 3 Tests (planned)

- All migrated callers produce identical results to Spur-based
- No regression in existing tests

## 9. Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Breaking existing callers | Step 1 adds new methods, doesn't remove old |
| Performance regression from double population | Both maps populated in same loop (O(1) extra per impl) |
| SubstsRef hashing overhead (Step 2) | Rc<[Ty]> hashes by content; acceptable for small substs |
| Generic parser not ready | Step 2 can wait; Step 1 is independently useful |

## 10. Unblocks

- **Task 11 (Monomorphization)**: Needs DefId + SubstsRef keys to
  instantiate generic functions with specific type arguments.
- **Task 14 (Object safety)**: Needs to check if a trait is object-safe,
  which requires knowing the trait's methods (via DefId).
- **Task 17 (Associated types)**: Needs DefId-keyed lookup to resolve
  associated type bindings.

## 11. References

- Stage 16.07 implementation: `docs/develop/v0/stage-15/stage-16.07-def-id-keyed-trait-impl-lookup.md`
- v0.3 roadmap: `docs/develop/v0/stage-15/stage-16.00-v0.3-kickoff.md`
- API naming standard: `docs/develop/v0/api-naming-standard.md`
- Stage committee process: `docs/stage-committee-process.md` §13.4, §16, §23
