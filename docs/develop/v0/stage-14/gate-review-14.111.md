# Stage 14.111 — Gate Review: UnificationTable HashMap→Vec Optimization

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.124.0 → v0.125.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 deep review)

## 1. Stage Summary

Stage 14.111 implements data structure audit recommendation #5: convert
`UnificationTable` from `HashMap<Vid, Binding>` to `Vec<Binding>` indexed
by `Vid.0 as usize`. This gives true O(1) lookup without hashing overhead.

## 2. What Was Done

### UnificationTable HashMap → Vec (src/typeck/unify.rs)

**Before**: Three `HashMap` stores:
- `ty_vars: HashMap<TyVid, Option<Ty>>`
- `int_vars: HashMap<IntVid, IntVarBinding>`
- `float_vars: HashMap<FloatVid, FloatVarBinding>`

Each lookup required hashing the `Vid` key + hash table traversal.

**After**: Three `Vec` stores:
- `ty_vars: Vec<Option<Ty>>`
- `int_vars: Vec<IntVarBinding>`
- `float_vars: Vec<FloatVarBinding>`

Indexed directly by `Vid.0 as usize` — true O(1) array indexing, no hashing.

### Why This Works

`TyVid(pub u32)`, `IntVid(pub u32)`, `FloatVid(pub u32)` are sequential IDs
allocated starting from 0 by `new_ty_var()` / `new_int_var()` / `new_float_var()`.
This makes them perfect for Vec indexing — no gaps, no sparse keys.

### Changes Made

- Removed `HashMap` import
- Removed `next_ty_vid` / `next_int_vid` / `next_float_vid` counters (Vec::len() replaces them)
- Updated `new_ty_var()` / `new_int_var()` / `new_float_var()` to use `push()`
- Updated all `get(&vid)` → `get(vid.0 as usize)`
- Updated all `insert(vid, val)` → `vec[vid.0 as usize] = val`
- Updated `default_unresolved()` to iterate `0..len()` instead of `.keys()`

### Performance Impact

- Eliminates hashing overhead for every type variable lookup (hundreds per function body)
- Better cache locality (Vec is contiguous, HashMap is not)
- No API change — all public methods have the same signatures
- Per §1.0 原则 6 "通用 > 特例": one Vec-per-store pattern handles all 3 variable kinds

## 3. Verification

```
cargo build --release --features llvm-backend: ✅
cargo fmt: ✅ clean
cargo clippy --all-targets --features llvm-backend: ✅ 0 warnings
cargo test --features llvm-backend: ✅ 1951 passed, 0 failed
python3 tests/conformance/run_all.py: ✅ 5216 passed, 0 failed
```

## 4. Data Structure Optimization Progress

| # | Optimization | Status | Stage |
|---|-------------|--------|-------|
| 4 | HirCrate Vec→indexed lookup | ✅ DONE | 14.110 |
| 5 | UnificationTable HashMap→Vec | ✅ DONE | 14.111 |
| 6 | Terminator → struct { kind, span } | ⏳ Pending | — |
| 7 | Consolidate 8 writeback passes | ⏳ Pending (v0.2) | — |
| 8 | dyn_trait_calls → Terminator::Call | ⏳ Pending (v0.2) | — |
| 1 | Intern Ty to Ty<'tcx> | ⏳ Pending (v0.2) | — |
| 10 | EmitValue = String → typed handle | ⏳ Pending (v0.2) | — |

## 5. Stage Verdict

**PASS** — UnificationTable converted from HashMap to Vec. All tests pass.
No regressions. True O(1) variable lookup without hashing overhead.

v0.125.0: minor bump (UnificationTable HashMap→Vec optimization)
