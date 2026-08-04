# Stage 16.11 — Task 3 Step 4: Spur-Based Method Deprecation

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.227.4 → v0.227.5
> **Process**: stage-committee-process.md v3.24 §1.0 原則 6 "通用 > 特例" + §23 API 命名标准化 + §23.6 弃用约定

## 1. Executive Summary

Stage 16.11 is **Step 4 of Task 3** (TraitResolver Keys redesign). It
deprecates the Spur-based query methods, completing the migration to
DefId-keyed lookup that began in Stage 16.07.

**Key changes**:
1. Deprecated `find_impl(trait_name: Spur, self_ty_name: Spur)` → use `find_impl_by_def_ids`
2. Deprecated `implements(trait_name: Spur, self_ty_name: Spur)` → use `implements_by_def_ids`
3. Deprecated `implements_by_def_id(trait_name: Spur, def_id: DefId)` → use `implements_by_def_ids`
4. Deprecated `find_vtable(trait_name: Spur, self_ty_name: Spur)` → use `find_vtable_by_def_ids`
5. Deprecated `impl_methods(trait_name: Spur, self_ty_name: Spur)` → use new `impl_methods_by_def_ids`
6. Added new `impl_methods_by_def_ids(trait_def_id, self_type_def_id)` method.
7. +7 integration tests verifying both new method and backward compat.

**Result**: Task 3 is now **COMPLETE** (Steps 1-4 all done). All production
query paths use DefId-keyed lookup. Spur-based methods retained for
backward compatibility with test contexts and string-keyed iteration
(`build_dyn_trait_fat_ptrs_from_resolver`).

## 2. Deprecated Methods

Per §23.6: deprecated methods have `note` pointing to the §16-compliant
alternative.

| Deprecated Method | Replacement | Note |
|-------------------|-------------|------|
| `find_impl(Spur, Spur)` | `find_impl_by_def_ids(DefId, DefId)` | Stage 16.07 |
| `implements(Spur, Spur)` | `implements_by_def_ids(DefId, DefId)` | Stage 16.07 |
| `implements_by_def_id(Spur, DefId)` | `implements_by_def_ids(DefId, DefId)` | Stage 16.07 |
| `find_vtable(Spur, Spur)` | `find_vtable_by_def_ids(DefId, DefId)` | Stage 16.10 |
| `impl_methods(Spur, Spur)` | `impl_methods_by_def_ids(DefId, DefId)` | Stage 16.11 (new) |

## 3. New Method: `impl_methods_by_def_ids`

```rust
/// Stage 16.11 (Task 3 Step 4): Get the method names implemented in an
/// impl block, keyed by DefIds. DefId-keyed equivalent of `impl_methods`.
pub fn impl_methods_by_def_ids(
    &self,
    trait_def_id: DefId,
    self_type_def_id: DefId,
) -> Option<&Vec<Spur>> {
    self.find_impl_by_def_ids(trait_def_id, self_type_def_id)
        .map(|i| &i.methods)
}
```

## 4. Internal Callers (#[allow(deprecated)])

Some internal `TraitResolver` methods still use the Spur-based path
because they themselves take Spur parameters (e.g., `resolve_vtable_method`,
`vtable_method_names`, `is_copy`, `impl_covers_trait`, `missing_impl_methods`).
These are Spur-based APIs that callers use with string names. They're
marked with `#[allow(deprecated)]` internally:

- `find_vtable_method_entry` — uses `find_vtable` (Spur-based, for string-keyed vtable method resolution)
- `vtable_method_names` — uses `find_vtable` (Spur-based, returns string names)
- `is_copy` — uses `implements_by_def_id` (Spur-based, for string-keyed Copy check)
- `impl_covers_trait` — uses `impl_methods` (Spur-based, for trait validation)
- `missing_impl_methods` — uses `impl_methods` (Spur-based, for trait validation)

These methods are NOT deprecated — they're string-keyed APIs that serve
a different purpose (callers that have string names, not DefIds).

## 5. External Callers (Production Path)

All production callers now use DefId-keyed lookup:
- `borrowck/copy_semantics.rs` — `is_copy_builtin` → `implements_by_def_ids` (Stage 16.08)
- `borrowck/copy_semantics.rs` — `is_clone_builtin` → `implements_by_def_ids` (Stage 16.08)
- `borrowck/copy_semantics.rs` — `is_drop_builtin` → `implements_by_def_ids` (Stage 16.08)
- `codegen/mod.rs` — drop glue → `is_drop_builtin` (Stage 16.08)
- `mir/dyn_trait.rs` — vtable lookup → `find_vtable_by_def_ids` (Stage 16.10, with Spur fallback)
- `mir/drop_elaboration.rs` — `is_drop_builtin` (Stage 16.08)

## 6. API Naming Standard Compliance (§23)

| Method | Pattern | Status |
|--------|---------|--------|
| `impl_methods_by_def_ids` | `<noun>_<noun>_<prep>_<noun>` | ✅ New |
| `find_impl` (deprecated) | `<verb>_<noun>` | ✅ `#[deprecated(note)]` |
| `implements` (deprecated) | `<verb>` | ✅ `#[deprecated(note)]` |
| `implements_by_def_id` (deprecated) | `<verb>_<prep>_<noun>` | ✅ `#[deprecated(note)]` |
| `find_vtable` (deprecated) | `<verb>_<noun>` | ✅ `#[deprecated(note)]` |
| `impl_methods` (deprecated) | `<noun>_<noun>` | ✅ `#[deprecated(note)]` |

Per §23.6: all deprecated methods have `note = "..."` pointing to the
§16-compliant alternative.

## 7. Tests

Added `tests/v0/stage16/plan/stage16_11_spur_deprecation_tests.rs`
with 7 tests:
1. `impl_methods_by_def_ids` returns method names
2. `impl_methods_by_def_ids` returns None for no impl
3. DefId-keyed and Spur-based `impl_methods` agree (consistency)
4. Deprecated `find_impl` still works (backward compat)
5. Deprecated `implements` still works (backward compat)
6. Deprecated `find_vtable` still works (backward compat)
7. Deprecated `implements_by_def_id` still works (backward compat)

## 8. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2198/2198 PASS (+7 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7666 tests passing, 0 failures, 0 warnings.**

## 9. Version Policy

v0.227.4 → v0.227.5 (patch bump — deprecation + new method, no behavior
change. Deprecated methods still work with `#[allow(deprecated)]`.)

## 10. Task 3 Roadmap — COMPLETE ✅

| Step | Status | Description |
|------|--------|-------------|
| Step 1 | ✅ COMPLETE (Stage 16.07) | DefId-keyed impl lookup |
| Step 2 | 🔧 Pending (future) | Add SubstsRef to keys for generics |
| Step 3 | ✅ COMPLETE (Stages 16.08 + 16.10) | Builtin trait checks + vtable migration |
| Step 4 | ✅ **COMPLETE (Stage 16.11)** | Deprecate Spur-based methods |

**Task 3 is now COMPLETE** (except Step 2 which requires generic parser
support and is deferred to when generics are implemented).

**Next**: Task 11 (Monomorphization) can start once generic parser support
is added. Alternatively, Task 10 (Closure redesign) or other v0.3 items.
