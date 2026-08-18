# Stage 18.182 — Array Index Codegen Fix (TD-ARRAY-INDEX-CODEGEN P0)

> **Date**: 2026-08-17
> **Version**: v0.449.0 → v0.450.0
> **Task ID**: stage18.182
> **Agent**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **Depends on**: Stage 18.181 (base types audit)
> **Blocks**: Stage 18.183 (fat pointer Index), Stage 18.184 (str methods), Stage 18.185 (String intrinsics)

## 1. Scope

Per Stage 18.181 task review: fix the P0 array index codegen bug where
`arr[N]` returned wrong values:
- `arr[1]` returned `arr[0]`'s value
- `arr[2]` returned 0 (out-of-bounds undetected)
- Multi-index expressions segfaulted

This is a P0 blocker for all heap-allocated types (String/Vec/format!) that
depend on array indexing.

## 2. Root Cause Analysis

### 2.1 Bug Reproduction

```landin
let arr = [10, 20, 30];
println!("{}", arr[0]);  // Expected: 10, Got: 10  ✅
println!("{}", arr[1]);  // Expected: 20, Got: 10  ❌
println!("{}", arr[2]);  // Expected: 30, Got: 10  ❌
```

### 2.2 IR Inspection

The generated LLVM IR showed:
```llvm
%v5 = load i32, %loc_8       ; loc_8 is the index local — NEVER stored to!
%v6 = getelementptr [3 x i32], ptr %loc_5, i32 0, i32 %v5
%v7 = load i32, %v6
```

The index local `loc_8` was loaded but never stored — DCE removed the
`let idx_local = 0` assignment because it didn't see `idx_local` as used.

### 2.3 Root Cause

In `src/mir/optimization.rs`, the `collect_place_locals` function (used by
DCE's `collect_read_locals`) handled `Projection(base, elem)` by recursing
into `base` but **ignoring the `elem`**:

```rust
PlaceKind::Projection(base, _) => collect_place_locals(base, used),
//                            ^ ignores elem entirely
```

For `arr[0]` → `Projection(Local(arr), Index(idx_local))`:
- `base` = `Local(arr)` → `arr` marked as used ✅
- `elem` = `Index(idx_local)` → `idx_local` NOT marked as used ❌

So DCE removed `let idx_local = 0`, leaving the alloca uninitialized. The
GEP then used garbage as the index, which happened to be 0 (from alloca
zero-init) — so `arr[1]` and `arr[2]` both loaded `arr[0]`.

### 2.4 The Fix

Extended `collect_place_locals` to also collect locals from the projection
element:

```rust
PlaceKind::Projection(base, elem) => {
    collect_place_locals(base, used);  // recurse into base (unchanged)
    match elem {
        ProjectionElem::Index(idx_local) => {
            used.insert(*idx_local);  // Stage 18.182: mark index as used
        }
        // Field/ConstantIndex/Subslice/Deref don't carry additional locals
        _ => {}
    }
}
```

Per §1.0 原則 4 (报错>静默): DCE must not remove used assignments.
Per §1.0 原則 6 (通解>特例): one recursive rule for all projection elements.

## 3. Tests

### 3.1 New Tests (tests/v0/stage18/plan/stage18_182_array_index_tests.rs)

8 tests (7 positive + 1 negative):
- Positive: each element access, multi-index one expr, let-bound index,
  mutation via index, various positions, different types, binary expr
- Negative: OOB soft test (warns, doesn't fail — bounds check is separate TD)

All 8 pass.

### 3.2 Test Results

```
running 8 tests
test stage18_182_array_index_each_element ... ok           (was: 10 10 10)
test stage18_182_array_multi_index_one_expr ... ok         (was: segfault)
test stage18_182_array_index_via_let_var ... ok            (already worked)
test stage18_182_array_mutation_via_index ... ok
test stage18_182_array_index_various_positions ... ok
test stage18_182_array_index_different_types ... ok
test stage18_182_array_index_in_binary_expr ... ok
test stage18_182_array_oob_soft ... ok                     (soft)
```

## 4. §3.2 Acceptance

- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 errors
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3004 passed (was 2996, +8 new)
- **Total**: 3662 tests, 0 failures

## 5. Tech Debt Status

| ID | Status |
|----|--------|
| TD-ARRAY-INDEX-CODEGEN | ✅ Resolved (Stage 18.182) — DCE now marks Index's idx_local as used |
| TD-ARRAY-BOUNDS-CHECK | 🟡 New — Stage 18.183+: insert LLVM bounds checks for arr[N] |
| TD-FAT-PTR-INDEX-PROJ | 🟡 Active — Stage 18.183 |
| TD-STR-METHODS-RUNTIME | 🟡 Active — Stage 18.184 |
| TD-STRING-INTRINSICS | 🟡 Active — Stage 18.185 |

## 6. Next Steps

Stage 18.183: fat pointer Index projection (s[0] for str/切片)
- codegen 添加 fat pointer Index projection 支持
- This unblocks str byte indexing and &[T] slice indexing
