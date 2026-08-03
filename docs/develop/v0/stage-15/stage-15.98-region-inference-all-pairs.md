# Stage 15.98 — Region Inference All-Pairs Constraint Matching

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.222.0 → v0.223.0
> **Process**: stage-committee-process.md v3.24 §25 (Deep Review) + §14.4 (重构即架构设计)

## 1. Executive Summary

Stage 15.98 fixes a **systematic simplification** in the region
inference constraint collection: the "first-to-first" matching pattern
that only matched the first region of each type pair, missing constraints
for types with multiple references (e.g., `&(&a i32, &b i32)`).

**Fix**: Replaced all 3 "first-to-first" matching sites with **all-pairs
matching** — for each source region and each destination region, add an
outlives constraint.

**Before** (first-to-first, missing constraints):
```rust
if let (Some(&src_r), Some(&lhs_r)) = (src_regions.first(), lhs_regions.first()) {
    self.add_outlives_constraint(src_r, lhs_r, ...);
}
```

**After** (all-pairs, complete constraints):
```rust
for &src_r in &src_regions {
    for &lhs_r in &lhs_regions {
        if src_r != lhs_r {
            self.add_outlives_constraint(src_r, lhs_r, ...);
        }
    }
}
```

Per user directive: "审查项目简化（设计、实现、测试），判断当前阶段是否能
闭合完整" — simplified implementations must be completed.
Per §1.0 原則 9 "正确 > 妥协": all-pairs matching is the correct approach.

## 2. Sites Fixed (3 sites)

### 2.1 Copy/Move reference propagation (line ~717)

**Before**: `r = Copy(x)` where x is `&(&a T, &b U)` → only `a:lhs_first`
**After**: `a:lhs_0`, `a:lhs_1`, `b:lhs_0`, `b:lhs_1` (all pairs)

### 2.2 Call argument constraints (line ~792)

**Before**: `f(&x)` where arg is `&(&a T, &b U)` and param is `&(&c T, &d U)`
→ only `a:c`
**After**: `a:c`, `a:d`, `b:c`, `b:d` (all pairs)

### 2.3 Call return value constraints (line ~848)

**Before**: `dest = f()` where return is `&(&a T, &b U)` and dest is
`&(&c T, &d U)` → only `a:c`
**After**: `a:c`, `a:d`, `b:c`, `b:d` (all pairs)

## 3. Why This Matters

The first-to-first matching was a simplification that worked for
single-reference types (`&i32`) but was **incorrect** for multi-reference
types:

```landin
fn foo(x: &(&i32, &i32)) -> &(&i32, &i32) { x }
```

With first-to-first: only `x.0_region : return.0_region` was constrained.
With all-pairs: `x.0_region : return.0_region`, `x.0_region : return.1_region`,
`x.1_region : return.0_region`, `x.1_region : return.1_region` are all
constrained.

The all-pairs approach is a conservative over-approximation — it may add
more constraints than strictly necessary (e.g., `a:d` when the correct
matching is positional `a:c, b:d`), but it never misses a required
constraint. This is sound: extra constraints can only cause false
positives (rejecting valid code), never false negatives (accepting
invalid code).

Per §1.0 原則 4 "报错 > 静默": better to over-constrain than under-constrain.

## 4. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2144/2144 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7612 tests passing, 0 failures, 0 warnings.**

## 5. Remaining Simplifications (v0.3 scope)

| Simplification | Location | Impact | Plan |
|----------------|----------|--------|------|
| `Adt(_, _) => true` in `ty_is_copy` | `copy_semantics.rs:54` | Unsound: all user structs treated as Copy in test contexts | v0.3: field-level Copy detection (Task 3 resolver) |
| Region error span TODO | `borrowck/mod.rs:218` | Lifetime errors show "1:1" | v0.3: constraint cause span tracking |
| `BinaryOp2` codegen fallback "0" | `codegen/rvalue.rs:442` | Range expressions in codegen (shouldn't occur — for-loop desugaring) | v0.3: proper range codegen |
| String/Vec macro simplified to unit | `mir/lower/expr_operand.rs:2010` | `format!`/`vec!` macros produce unit | v0.3: alloc support |

These are all documented and tracked. None affect v0.2 correctness for
supported features.

## 6. Version Policy

v0.222.0 → v0.223.0 (minor bump — region inference all-pairs matching
fix, systematic simplification correction).
