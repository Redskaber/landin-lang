# Stage 16.50 — Task 11 Phase 1a: `generics_of` Query

> **Author**: redskaber
> **Date**: 2026-08-04
> **Version**: v0.236.0 → v0.236.1
> **Process**: stage-committee-process.md v3.24 §13.4 + §23

## 1. Executive Summary

Stage 16.50 implements Task 11 Phase 1a — the `generics_of` query that maps
a `DefId` to its type parameters (`Vec<ParamTy>`). This is the foundation
for monomorphization: knowing how many type parameters a generic item has.

**What was created**:
- `src/hir/generics.rs` — new module with:
  - `build_generics_map(hir) -> HashMap<DefId, Vec<ParamTy>>` — build full map
  - `generics_of(def_id, hir) -> Vec<ParamTy>` — query single item
  - `extract_type_params(owner) -> Option<Vec<ParamTy>>` — extract from HIR
- 6 unit tests verifying:
  - Non-generic fn → empty params
  - Generic struct (Pair<A,B>) → 2 params with correct indices
  - build_generics_map collects all generic items
  - Non-generic struct excluded from map
  - Generic fn (id<T>) → 1 param
  - Lifetime params skipped (only type params counted)

**Test results**: 7911 tests passing (250 lib + 2437 integration + 5224
conformance), 0 failures, 0 warnings.

## 2. API (§23 Compliant)

```rust
// In src/hir/generics.rs

/// Build a map from DefId → Vec<ParamTy> for all generic items.
/// Per §23: `build_generics_map` follows `<verb>_<noun>_<noun>` pattern.
pub fn build_generics_map(hir: &HirCrate) -> HashMap<DefId, Vec<ParamTy>>

/// Query: get type parameters for a given DefId.
/// Per §23: `generics_of` follows `<noun>_<prep>` pattern (query function).
pub fn generics_of(def_id: DefId, hir: &HirCrate) -> Vec<ParamTy>
```

## 3. How It Works

1. Walk all `hir.owners` (Vec of `(DefId, OwnerNode)`)
2. For each owner, extract `HirGenerics` from the item (fn/struct/enum/trait/impl/type-alias)
3. Walk `generics.params`, filter for `HirGenericParam::Type` (skip lifetimes)
4. Convert each `HirTypeParam` to `ParamTy { index, name }`
5. `index` = position among type params (0-based, lifetimes skipped)

## 4. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 250/250 PASS (+6 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2437/2437 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7911 tests passing, 0 failures, 0 warnings.**

## 5. Version Policy

v0.236.0 → v0.236.1 (patch bump — new module + query function, no existing
API changes.)
