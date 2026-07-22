# Stage 5 Development Log

> **Author**: redskaber
> **Date**: 2026-07-22
> **Version**: v0.11.0
> **Status**: 🔄 In progress (5.1 complete, 5.2+ pending)

## Overview

Stage 5 focuses on: TraitResolver, vtable generation, stdlib MVP, mini-cargo,
user-defined macros, and NLL fixpoint. This stage was launched after the
cross-stage deep review R49 (GO for Stage 5).

## Sub-stages

### Stage 5.1 — TraitResolver 基础 (v0.11.0)

**Priority**: Stage 5 core — trait resolution infrastructure.

**Work completed**:
- New `src/traits/mod.rs` — TraitResolver module
  * `TraitInfo` — trait definition metadata (def_id, name, methods, is_unsafe)
  * `ImplInfo` — impl block metadata (def_id, trait_name, self_ty_name, methods, is_unsafe)
  * `TraitResolver` — collects trait/impl from HIR, builds dispatch tables
  * `trait_by_name` — Spur → DefId lookup
  * `impl_by_trait_and_type` — (trait_name, self_ty_name) → DefId lookup
  * `find_trait` / `find_impl` / `implements` — query methods
- `src/lib.rs` — added `pub mod traits` + `pub use traits::TraitResolver`
- 3 new tests in `tests/v0/stage5/plan/trait_resolver_tests.rs`

**Test impact**: +3 (1005/1005 — was 1002)
**Verification**: 0 clippy warnings, fmt clean

### Stage 5.2 — TraitResolver Driver Integration + fmt Fix (v0.11.1)

**Priority**: Integrate TraitResolver into pipeline + fix fmt issues.

**Work completed**:
- src/driver.rs: CompileResult now has `trait_resolver: TraitResolver` field
- compile() builds TraitResolver via `collect(&hir, &interner)` after resolve
- CompileResult::empty() initializes empty TraitResolver
- Fixed cargo fmt issues in src/traits/mod.rs + tests/v0/stage5/plan/trait_resolver_tests.rs
- 2 new integration tests in tests/v0/stage5/plan/trait_integration_tests.rs

**Test impact**: +2 (1007/1007 — was 1005)
**Verification**: 0 clippy warnings, **fmt clean (zero diff)** ✅

### Stage 5.3 — ty_is_copy_with_resolver (v0.11.2)

**Priority**: Precise Copy detection using TraitResolver.

**Work completed**:
- src/borrowck/mod.rs: new `ty_is_copy_with_resolver(ty, resolver, interner)` function
  * For non-Adt types: identical to ty_is_copy
  * For Adt: falls back to true (same as ty_is_copy) until DefId→name map (Stage 5.4)
  * Recursive for Tuple and Array
- Original `ty_is_copy` retained as fallback (no resolver needed)
- 3 new tests in tests/v0/stage5/plan/ty_is_copy_tests.rs

**Test impact**: +3 (1010/1010 — was 1007)
**Verification**: 0 clippy warnings, **fmt clean (exit 0)** ✅

### Stage 5.4 — DefId→name Reverse Map + Full Copy Detection (v0.11.3)

**Priority**: Complete Copy trait detection — close TD-016.

**Work completed**:
- src/traits/mod.rs: added `type_by_def_id: HashMap<DefId, Spur>` field
  * Populated for struct/enum/trait during `collect()`
  * New query methods: `implements_by_def_id()`, `is_copy()`, `type_count()`
- src/borrowck/mod.rs: `ty_is_copy_with_resolver` Adt branch now fully active
  * Looks up type name via `type_by_def_id`
  * Checks `resolver.is_copy(def_id, copy_name)` — returns false if no Copy impl
  * Falls back to true if "Copy" not interned (conservative)
- 3 new tests in tests/v0/stage5/plan/def_id_name_map_tests.rs

**TD-016 status**: ✅ CLOSED — Copy detection now uses TraitResolver instead of
treating all Adt as Copy.

**Test impact**: +3 (1013/1013 — was 1010)
**Verification**: 0 clippy warnings, **fmt clean (exit 0)** ✅
